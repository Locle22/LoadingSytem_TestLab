# =============================================================================
# LoadingSystem HMIv2 - Main Application (Dark Neon Industrial Theme)
# =============================================================================
# Giao diện HMI v2 mô phỏng 3D Không gian 4 phân hệ với Dark Neon Theme:
#   (1) Ray tách muỗi 3D (Khay nghiêng + Camera AI Vision Sensor)
#   (2) Băng chuyền 3D (Animated chevrons + 2 bánh răng neon)
#   (3) Cần gạt 3D cấp 1-3
#   (4) Đĩa xoay 3D (12 LỌ 12 LOÀI MUỖI - Neon Glow)
#
# Tích hợp: Thuật toán Thả 12 loài ngẫu nhiên, Auto-Speed Max 1s,
# Easing Animation 60fps, Particle Effects, Neon Glow UI.
# =============================================================================

import datetime
import random
import threading
import customtkinter as ctk
from typing import Optional, Dict

from app.zmq_client import IpcClient
from app.system_visualizer import SystemVisualizer
from app.control_panel import ControlPanel
from app.override_panel import OverridePanel
from app.animation_engine import NeonTheme, PulseGlow, neon_glow_color


class LoadingSystemHMI(ctk.CTk):
    """
    Cửa sổ chính HMIv2 LoadingSystem (Dark Neon Industrial Theme 60fps).
    """

    APP_TITLE = "LoadingSystem HMIv2 - Mosquito Machine 3D (Ray ➔ Conveyor ➔ Chute ➔ 12-Jar Dynamic Carousel)"
    APP_WIDTH = 1000
    APP_HEIGHT = 860

    def __init__(
        self,
        backend_host: str = "127.0.0.1",
        backend_port: int = 5555,
    ):
        super().__init__()

        # Cấu hình cửa sổ
        self.title(self.APP_TITLE)
        self.geometry(f"{self.APP_WIDTH}x{self.APP_HEIGHT}")
        self.minsize(960, 780)
        self.resizable(True, True)

        # Dark Neon Theme
        ctk.set_appearance_mode("dark")
        ctk.set_default_color_theme("blue")
        self.configure(fg_color=NeonTheme.BG_DEEP)

        # IPC client
        self._client = IpcClient(host=backend_host, port=backend_port)
        self._current_angle = 0.0

        # Auto Feed Loop State
        self._auto_loop_running = False
        self._auto_loop_timer = None

        # Connection pulse glow
        self._conn_pulse = PulseGlow(speed=2.0, min_val=0.3, max_val=1.0)

        # Xây dựng giao diện
        self._build_ui()

        # Kết nối callback bảng đếm muỗi & gán nhãn
        self._sys_viz.set_count_change_callback(self._override_panel.update_jar_counts)

        # Log khởi tạo
        self.log_msg("SYSTEM", "HMIv2 Đóng Lọ Muỗi 3D đã khởi tạo thành công. (Dark Neon Theme 60fps)")
        self.log_msg("CONFIG", "Thuật toán điều tốc tự động & Phân loại 12 loài muỗi đã kích hoạt.")

        # Tự động kết nối khi khởi động
        self.after(500, self._auto_connect)

        # Handle đóng cửa sổ
        self.protocol("WM_DELETE_WINDOW", self._on_close)

    def _build_ui(self):
        """Xây dựng toàn bộ giao diện HMIv2 Dark Neon"""

        # ---- Header Neon ----
        header = ctk.CTkFrame(self, fg_color=NeonTheme.HEADER_BG, height=48, corner_radius=0)
        header.pack(fill="x")
        header.pack_propagate(False)

        # Neon accent line top
        accent_line = ctk.CTkFrame(header, fg_color=NeonTheme.NEON_CYAN, height=2, corner_radius=0)
        accent_line.pack(fill="x", side="top")

        ctk.CTkLabel(
            header,
            text="⚡ HMIv2 — HỆ THỐNG ĐÓNG LỌ MUỖI 3D — THUẬT TOÁN ĐIỀU TỐC TỰ ĐỘNG (MAX 1S)",
            font=("Segoe UI", 14, "bold"),
            text_color=NeonTheme.NEON_CYAN,
        ).pack(side="left", padx=16, pady=8)

        # Connection indicator neon
        self._conn_frame = ctk.CTkFrame(header, fg_color="transparent")
        self._conn_frame.pack(side="right", padx=16)

        self._conn_dot = ctk.CTkLabel(
            self._conn_frame,
            text="●",
            font=("Segoe UI", 14),
            text_color=NeonTheme.NEON_RED,
        )
        self._conn_dot.pack(side="left", padx=(0, 4))

        self._conn_label = ctk.CTkLabel(
            self._conn_frame,
            text="Ngắt kết nối",
            font=("Segoe UI", 10, "bold"),
            text_color=NeonTheme.TEXT_DIM,
        )
        self._conn_label.pack(side="left")

        # ---- Main content frame ----
        content = ctk.CTkFrame(self, fg_color="transparent")
        content.pack(fill="both", expand=True, padx=8, pady=4)

        # ---- System Visualizer 3D Neon (Top Section) ----
        viz_frame = ctk.CTkFrame(
            content,
            fg_color=NeonTheme.BG_PANEL,
            border_color=NeonTheme.BORDER_DEFAULT,
            border_width=1,
            corner_radius=10,
        )
        viz_frame.pack(fill="x", pady=(0, 4))

        self._sys_viz = SystemVisualizer(viz_frame, width=960, height=350)
        self._sys_viz.pack(pady=4, padx=4)

        # ---- Middle Section: Control Panels (Side-by-Side) ----
        mid_frame = ctk.CTkFrame(content, fg_color="transparent")
        mid_frame.pack(fill="x", pady=(0, 4))
        mid_frame.grid_columnconfigure((0, 1), weight=1)

        # Control Panel (Left)
        self._control_panel = ControlPanel(
            mid_frame,
            on_rotate_right=self._cmd_rotate_right,
            on_rotate_left=self._cmd_rotate_left,
            on_return_home=self._cmd_return_home,
            on_feed_nut=self._cmd_feed_nut,
            on_toggle_auto_loop=self._cmd_toggle_auto_loop,
            on_confirm_config=self._cmd_confirm_config,
        )
        self._control_panel.grid(row=0, column=0, padx=(0, 2), sticky="nsew")

        # Override Panel (Right)
        self._override_panel = OverridePanel(
            mid_frame,
            on_override=self._cmd_override,
        )
        self._override_panel.grid(row=0, column=1, padx=(2, 0), sticky="nsew")

        # ---- Bottom Section: LOG CONSOLE NEON ----
        log_frame = ctk.CTkFrame(
            content,
            fg_color=NeonTheme.BG_PANEL,
            border_color=NeonTheme.BORDER_DEFAULT,
            border_width=1,
            corner_radius=8,
        )
        log_frame.pack(fill="both", expand=True)

        log_title_box = ctk.CTkFrame(log_frame, fg_color=NeonTheme.BG_CARD, height=24, corner_radius=0)
        log_title_box.pack(fill="x")
        log_title_box.pack_propagate(False)

        ctk.CTkLabel(
            log_title_box,
            text="📋 NHẬT KÝ VẬN HÀNH THỜI GIAN THỰC (REAL-TIME LOG CONSOLE)",
            font=("Segoe UI", 9, "bold"),
            text_color=NeonTheme.NEON_CYAN,
        ).pack(side="left", padx=8, pady=2)

        self.btn_clear_log = ctk.CTkButton(
            log_title_box,
            text="Xóa Log",
            font=("Segoe UI", 9),
            height=18,
            width=60,
            fg_color=NeonTheme.SURFACE_2,
            hover_color=NeonTheme.SURFACE_3,
            text_color=NeonTheme.TEXT_SECONDARY,
            command=self._clear_log,
        )
        self.btn_clear_log.pack(side="right", padx=6, pady=2)

        # Log Text Box Neon
        self.log_textbox = ctk.CTkTextbox(
            log_frame,
            font=("Consolas", 10),
            fg_color=NeonTheme.LOG_BG,
            text_color=NeonTheme.TEXT_PRIMARY,
            corner_radius=4,
            wrap="word",
        )
        self.log_textbox.pack(fill="both", expand=True, padx=4, pady=4)

        # Configure Neon Tag Colors
        for tag_name, tag_color in NeonTheme.LOG_TAGS.items():
            self.log_textbox.tag_config(tag_name, foreground=tag_color)

        # ---- Status Bar Neon ----
        self._status_bar = ctk.CTkFrame(self, fg_color=NeonTheme.STATUS_BG, height=26, corner_radius=0)
        self._status_bar.pack(fill="x", side="bottom")
        self._status_bar.pack_propagate(False)

        # Neon accent line on status bar
        status_accent = ctk.CTkFrame(self._status_bar, fg_color=NeonTheme.NEON_CYAN, height=1, corner_radius=0)
        status_accent.pack(fill="x", side="top")

        self._status_label = ctk.CTkLabel(
            self._status_bar,
            text="HMIv2 sẵn sàng vận hành.",
            font=("Segoe UI", 9),
            text_color=NeonTheme.TEXT_SECONDARY,
        )
        self._status_label.pack(side="left", padx=12, pady=2)

        self._connect_btn = ctk.CTkButton(
            self._status_bar,
            text="Kết nối",
            font=("Segoe UI", 9, "bold"),
            fg_color=NeonTheme.SURFACE_2,
            hover_color=NeonTheme.SURFACE_3,
            text_color=NeonTheme.NEON_CYAN,
            height=20,
            width=80,
            corner_radius=4,
            border_width=1,
            border_color=NeonTheme.NEON_CYAN,
            command=self._toggle_connection,
        )
        self._connect_btn.pack(side="right", padx=10, pady=2)

        # Disable controls initially
        self._control_panel.set_enabled(False)
        self._override_panel.set_enabled(False)

    # =========================================================================
    # Logging System (Neon Tags)
    # =========================================================================

    def log_msg(self, tag: str, message: str):
        """Ghi log có dấu thời gian và phân màu neon"""
        now = datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3]

        self.log_textbox.configure(state="normal")
        self.log_textbox.insert("end", f"[{now}] ", "TIME")
        tag_style = tag if tag in NeonTheme.LOG_TAGS else "INFO"
        self.log_textbox.insert("end", f"[{tag:<8}] ", tag_style)
        self.log_textbox.insert("end", f"{message}\n")
        self.log_textbox.see("end")
        self.log_textbox.configure(state="disabled")

    def _clear_log(self):
        self.log_textbox.configure(state="normal")
        self.log_textbox.delete("1.0", "end")
        self.log_textbox.configure(state="disabled")

    # =========================================================================
    # Connection Management
    # =========================================================================

    def _auto_connect(self):
        self._set_status("Đang kết nối tới Rust Backend Daemon (127.0.0.1:5555)...")
        threading.Thread(target=self._connect_async, daemon=True).start()

    def _connect_async(self):
        success = self._client.connect()
        self.after(0, lambda: self._on_connect_result(success))

    def _on_connect_result(self, success: bool):
        if success:
            self._conn_dot.configure(text_color=NeonTheme.NEON_GREEN)
            self._conn_label.configure(text="Đã kết nối", text_color=NeonTheme.NEON_GREEN)
            self._connect_btn.configure(text="Ngắt kết nối")
            self._control_panel.set_enabled(True)
            self._override_panel.set_enabled(True)
            self._set_status("✓ Đã kết nối thành công tới Rust Backend Server (127.0.0.1:5555)")
            self.log_msg("INFO", "Kết nối Socket TCP IPC thành công tới Rust Backend Daemon.")
            self._refresh_status()
        else:
            self._conn_dot.configure(text_color=NeonTheme.NEON_RED)
            self._conn_label.configure(text="Ngắt kết nối", text_color=NeonTheme.TEXT_DIM)
            self._connect_btn.configure(text="Kết nối")
            self._control_panel.set_enabled(False)
            self._override_panel.set_enabled(False)
            self._stop_auto_loop()
            self._set_status("✗ Mất kết nối — Hãy kiểm tra daemon Rust backend đang chạy!")
            self.log_msg("ERROR", "Không thể kết nối IPC Server (127.0.0.1:5555). Hãy khởi chạy Rust Backend.")

    def _toggle_connection(self):
        if self._client.is_connected:
            self._client.disconnect()
            self._on_connect_result(False)
        else:
            self._auto_connect()

    # =========================================================================
    # MOSQUITO DISPENSING & AUTOMATIC SPEED CALCULATION ALGORITHM
    # =========================================================================

    def _cmd_feed_nut(self) -> float:
        """
        THẢ RANDOM 12 LOÀI MUỖI VÀO 12 LỌ RIÊNG BIỆT:
        - Mỗi lọ chỉ chứa duy nhất 1 loài muỗi (1-to-1 mapping).
        - Đĩa tự động tính toán tần số f (Hz) và quay đúng vị trí trong MAX 1S (hoặc t_drop).
        
        Returns:
            rot_duration: Thời gian quay motor tính bằng giây
        """
        # 1. Chọn ngẫu nhiên 1 trong 12 loài muỗi
        incoming_species = random.randint(1, 12)
        sp_name = SystemVisualizer.SPECIES_NAMES[incoming_species - 1]

        # 2. Lọ đích cố định đại diện cho loài muỗi này (Lọ 1..12)
        target_jar = self._sys_viz.species_assigned_jar.get(incoming_species, 1)

        # 3. Tính toán vị trí góc chuẩn xác trước khi xoay và điểm rơi mục tiêu
        start_jar = self._sys_viz.get_active_jar_index()
        start_angle = self._current_angle

        # Tính bước di chuyển ngắn nhất theo lọ (-6 đến +6)
        jar_diff = target_jar - start_jar
        if jar_diff > 6:
            jar_diff -= 12
        elif jar_diff < -6:
            jar_diff += 12

        # Góc cần quay delta_deg (-180° đến +180°)
        delta_deg = -jar_diff * 30.0
        target_angle = start_angle + delta_deg

        # 4. THUẬT TOÁN TÍNH TỐC ĐỘ TỰ ĐỘNG (Auto-Speed Algorithm)
        opt_freq, rot_duration = self._control_panel.calculate_optimal_frequency(delta_deg)
        t_drop = self._control_panel.get_physical_drop_time()

        # 5. GHI LOG
        log_detail = (
            f"🎲 Phễu Nạp: [{sp_name}] ➔ Mục tiêu: Lọ #{target_jar}\n"
            f"   ├─ Vị trí trước: Lọ #{start_jar} ({start_angle:.1f}°) ➔ Điểm rơi: Lọ #{target_jar} ({target_angle:.1f}°)\n"
            f"   ├─ Góc cần quay: Δθ = {delta_deg:+.1f}° | Tần số Servo: f = {opt_freq} Hz\n"
            f"   └─ Thời gian quay Motor: {rot_duration:.2f}s | Thời gian muỗi rơi t_drop: {t_drop:.2f}s"
        )
        self.log_msg("DISPENSE", log_detail)
        self._set_status(f"Thả Muỗi [{sp_name}]: Xoay {delta_deg:+.1f}° về Lọ #{target_jar} ({opt_freq} Hz, {rot_duration:.2f}s)...")

        # 6. Gửi lệnh quay Servo kèm tần số tối ưu xuống Rust/PLC
        if abs(delta_deg) > 0.1:
            if delta_deg > 0:
                self._send_command_async("ROTATE_RIGHT", abs(delta_deg), frequency=opt_freq)
            else:
                self._send_command_async("ROTATE_LEFT", abs(delta_deg), frequency=opt_freq)
            # Cập nhật đĩa 3D HMI xoay mượt với easing
            self._sys_viz.update_angle(target_angle, duration_sec=rot_duration)

        # 7. Mô phỏng phôi muỗi 3D di chuyển
        self._sys_viz.start_muoi_flow_simulation(
            species_id=incoming_species,
            on_complete=lambda: self._set_status(f"✓ Muỗi [{sp_name}] đã rơi trúng Lọ #{target_jar}!")
        )

        return rot_duration

    def _cmd_confirm_config(self, freq: int, t_drop: float):
        msg = f"✔ ĐÃ XÁC NHẬN CẤU HÌNH: Tần số Servo = {freq} Hz | t_drop = {t_drop:.2f}s"
        self.log_msg("CONFIG", msg)
        self._set_status(f"✔ Đã áp dụng cấu hình mới: {freq} Hz, t_drop={t_drop:.2f}s")

    # =========================================================================
    # Continuous Auto Feed Loop
    # =========================================================================

    def _cmd_toggle_auto_loop(self, running: bool):
        if running:
            self._auto_loop_running = True
            self.log_msg("CONFIG", "▶ ĐÃ BẬT VÒNG LẶP TỰ ĐỘNG THẢ MUỖI LIÊN TỤC.")
            self._set_status("▶ ĐANG CHẠY VÒNG LẶP TỰ ĐỘNG THẢ MUỖI LIÊN TỤC...")
            self._auto_feed_step()
        else:
            self._stop_auto_loop()

    def _auto_feed_step(self):
        if not self._auto_loop_running or not self._client.is_connected:
            return

        rot_duration = self._cmd_feed_nut()
        t_drop = self._control_panel.get_physical_drop_time()

        # Tính toán thời gian chờ động: thời gian motor quay + thời gian muỗi rơi + buffer an toàn 1.2s
        wait_sec = max(3.5, rot_duration + t_drop + 1.2)
        interval_ms = int(wait_sec * 1000)

        self._auto_loop_timer = self.after(interval_ms, self._auto_feed_step)

    def _stop_auto_loop(self):
        self._auto_loop_running = False
        if self._auto_loop_timer:
            self.after_cancel(self._auto_loop_timer)
            self._auto_loop_timer = None
        self._control_panel.set_auto_loop_state(False)
        self.log_msg("CONFIG", "⏹ ĐÃ DỪNG VÒNG LẶP TỰ ĐỘNG THẢ MUỖI.")
        self._set_status("⏹ Đã dừng vòng lặp tự động thả muỗi.")

    # =========================================================================
    # Servo Command Handlers
    # =========================================================================

    def _send_command_async(self, command: str, value: Optional[float] = None, frequency: Optional[int] = None):
        def _run():
            try:
                response = self._client.send_command(command, value, frequency)
                self.after(0, lambda: self._on_command_response(response))
            except ConnectionError as e:
                err_msg = str(e)
                self.after(0, lambda: self._on_command_error(err_msg))

        threading.Thread(target=_run, daemon=True).start()

    def _on_command_response(self, response: dict):
        success = response.get("success", False)
        message = response.get("message", "Unknown response")
        if not success:
            self.log_msg("ERROR", f"Lỗi phản hồi PLC: {message}")
            self._set_status(f"✗ Lỗi PLC: {message}")
        self._refresh_status()

    def _on_command_error(self, error: str):
        self.log_msg("ERROR", f"Lỗi Socket IPC: {error}")
        self._set_status(f"✗ Lỗi IPC: {error}")
        self._on_connect_result(False)

    def _cmd_rotate_right(self, degrees: float, freq: int = 8192):
        self.log_msg("ROTATE", f"Lệnh quay PHẢI {degrees:.1f}° (Tần số: {freq} Hz)")
        self._set_status(f"Đang quay phải đĩa xoay {degrees:.1f}° ({freq} Hz)...")
        self._send_command_async("ROTATE_RIGHT", degrees, frequency=freq)
        self._sys_viz.update_angle(self._current_angle + degrees, duration_sec=0.5)

    def _cmd_rotate_left(self, degrees: float, freq: int = 8192):
        self.log_msg("ROTATE", f"Lệnh quay TRÁI {degrees:.1f}° (Tần số: {freq} Hz)")
        self._set_status(f"Đang quay trái đĩa xoay {degrees:.1f}° ({freq} Hz)...")
        self._send_command_async("ROTATE_LEFT", degrees, frequency=freq)
        self._sys_viz.update_angle(self._current_angle - degrees, duration_sec=0.5)

    def _cmd_return_home(self):
        self.log_msg("ROTATE", "Lệnh quay VỀ GỐC 0°")
        self._set_status("Đang quay đĩa xoay về vị trí gốc 0°...")
        self._send_command_async("RETURN_HOME")
        self._sys_viz.update_angle(0.0, duration_sec=0.6)

    def _cmd_override(self, degrees: float):
        direction = "phải" if degrees > 0 else "trái"
        self.log_msg("CONFIG", f"Hiệu chỉnh thủ công Fine Tune: {degrees:+.1f}° sang {direction}")
        self._set_status(f"Hiệu chỉnh thủ công Level 4/5: {degrees:+.1f}° sang {direction}...")
        self._send_command_async("OVERRIDE_ADJUST", degrees)

    # =========================================================================
    # Status Updates
    # =========================================================================

    def _refresh_status(self):
        def _run():
            try:
                response = self._client.get_status()
                self.after(0, lambda: self._update_status_display(response))
            except ConnectionError:
                pass

        threading.Thread(target=_run, daemon=True).start()

    def _update_status_display(self, response: dict):
        if not response.get("success"):
            return

        data = response.get("data", {})
        if not data:
            return

        angle = data.get("current_angle", 0.0)
        self._current_angle = angle

        self._override_panel.update_calibration_info(
            offset_pulses=data.get("offset_pulses", 0),
            learning_coefficient=data.get("learning_coefficient", 0.1),
            calibration_count=data.get("calibration_count", 0),
        )

    def _set_status(self, message: str):
        self._status_label.configure(text=message)

    def _on_close(self):
        self._stop_auto_loop()
        if self._client.is_connected:
            self._client.disconnect()
        self.destroy()
