# =============================================================================
# LoadingSystem HMIv2 - System Flow Visualizer (Dark Neon Industrial Theme)
# =============================================================================
# Mô phỏng 3D Không gian 4 phân hệ đóng lọ muỗi với hiệu ứng Neon 60fps:
#   (1) Ray tách muỗi 3D (Khay nghiêng + Phễu nạp 3D + Camera AI)
#   (2) Băng chuyền 3D (Animated chevron + 2 bánh răng 3D neon)
#   (3) Cần gạt 3D cấp 1-3
#   (4) ĐĨA XOAY 3D MANG 12 LỌ TỰ ĐỘNG - NEON GLOW EFFECTS
#
# 60fps Animation: Easing curves, neon trails, particle scatter, idle glow
# =============================================================================

import math
import time
import random
import customtkinter as ctk
from typing import Optional, Callable, Dict, List

from app.animation_engine import (
    AnimatedValue, PulseGlow, ParticleSystem, NeonDraw, NeonTheme,
    ease_in_out_cubic, ease_out_cubic, ease_out_elastic,
    dim_color, neon_glow_color, lerp_color
)


class SystemVisualizer(ctk.CTkCanvas):
    """
    Canvas mô phỏng 3D Dark Neon 4 phân hệ đóng lọ muỗi với 12 lọ - 60fps engine.
    """

    # Danh sách 12 Loài Muỗi
    SPECIES_NAMES = [
        "1. Aedes aegypti (Vằn)",
        "2. Anopheles (Sốt rét)",
        "3. Culex quinquefasciatus",
        "4. Mansonia uniformis",
        "5. Aedes albopictus",
        "6. Anopheles minimus",
        "7. Culex tritaeniorhynchus",
        "8. Armigeres subalbatus",
        "9. Coquillettidia",
        "10. Toxorhynchites",
        "11. Culiseta annulata",
        "12. Psorophora ferox"
    ]

    SPECIES_COLORS = NeonTheme.SPECIES_COLORS

    def __init__(self, master, width: int = 960, height: int = 350, **kwargs):
        super().__init__(
            master,
            width=width,
            height=height,
            bg=NeonTheme.BG_CANVAS,
            highlightthickness=0,
            **kwargs,
        )
        self.canvas_width = width
        self.canvas_height = height

        # Tọa độ 4 phân hệ theo trục 3D Perspective
        self.ray_x_start = 35
        self.ray_x_end = 200
        self.ray_y = 170

        self.belt_x_start = 220
        self.belt_x_end = 520
        self.belt_y = 170

        self.chute_x_start = 520
        self.chute_x_end = 585

        self.disc_cx = 730
        self.disc_cy = 185
        self.disc_rx = 120
        self.disc_ry = 60

        # ---- Animation State ----
        self._anim_angle = AnimatedValue(0.0)
        self._current_angle = 0.0
        self._target_angle = 0.0

        # Muoi flow state
        self._muoi_phase = "DISC"
        self._muoi_progress = 1.0
        self._muoi_present = True
        self._animating_muoi = False
        self._active_species_id = 1

        # Continuous idle animations
        self._sprocket_angle = 0.0
        self._chevron_offset = 0.0
        self._laser_sweep_t = 0.0
        self._idle_glow_phase = 0.0
        self._scanner_active = True

        # Pulse glow controllers
        self._disc_glow = PulseGlow(speed=1.5, min_val=0.2, max_val=0.8)
        self._active_jar_glow = PulseGlow(speed=3.0, min_val=0.3, max_val=1.0)
        self._scanner_glow = PulseGlow(speed=2.5, min_val=0.1, max_val=0.7)

        # Particle system
        self._particles = ParticleSystem()

        # 12 Lọ mapping
        self.jar_counts: Dict[int, int] = {i: 0 for i in range(1, 13)}
        self.jar_assigned_species: Dict[int, Optional[int]] = {i: i for i in range(1, 13)}
        self.species_assigned_jar: Dict[int, Optional[int]] = {i: i for i in range(1, 13)}

        self._on_count_change: Optional[Callable] = None
        self._on_flow_complete: Optional[Callable] = None

        # Timing
        self._last_frame_time = time.perf_counter()
        self._frame_count = 0

        # Start 60fps render loop
        self._start_render_loop()

    # =========================================================================
    # 60fps Render Loop
    # =========================================================================

    def _start_render_loop(self):
        """Khởi động vòng lặp render 60fps liên tục"""
        self._last_frame_time = time.perf_counter()
        self._render_frame()

    def _render_frame(self):
        """Main render frame — chạy liên tục ở ~60fps"""
        now = time.perf_counter()
        dt = min(now - self._last_frame_time, 0.05)  # Cap dt 50ms
        self._last_frame_time = now

        # Update animations
        self._update_animations(dt)

        # Redraw canvas
        self._redraw()

        # Schedule next frame (~16ms = 60fps)
        self.after(16, self._render_frame)

    def _update_animations(self, dt: float):
        """Cập nhật tất cả animation state mỗi frame"""
        # Disc angle easing
        if self._anim_angle.is_animating:
            self._anim_angle.update(dt)
            self._current_angle = self._anim_angle.current

        # Idle animations (luôn chạy)
        self._sprocket_angle += 45.0 * dt  # Sprocket quay chậm liên tục
        self._chevron_offset += 60.0 * dt  # Chevron di chuyển
        self._laser_sweep_t += dt * 1.2     # Laser sweep
        self._idle_glow_phase += dt

        # Muoi flow animation
        if self._animating_muoi:
            self._update_muoi_flow(dt)

        # Particles
        self._particles.update(dt)

    # =========================================================================
    # Physical Servo Rotation Speed Sync
    # =========================================================================

    def update_angle(self, angle: float, duration_sec: float = 0.5):
        """
        Cập nhật góc quay đĩa 3D với easing animation đồng bộ Motor thực tế.
        """
        self._target_angle = angle
        diff = angle - self._current_angle
        if abs(diff) > 0.01 and duration_sec > 0.05:
            self._anim_angle.current = self._current_angle
            self._anim_angle.animate_to(angle, duration=duration_sec, easing=ease_in_out_cubic)
        else:
            self._current_angle = angle
            self._anim_angle.set_immediate(angle)

    # =========================================================================
    # Flow & Species Assignment Logic
    # =========================================================================

    def start_muoi_flow_simulation(
        self,
        species_id: int = 1,
        on_complete: Optional[Callable[[], None]] = None
    ):
        """Khởi chạy mô phỏng 1 con muỗi thuộc loài species_id rơi vào lọ hứng"""
        if self._animating_muoi:
            return

        self._active_species_id = species_id
        self._muoi_phase = "RAY"
        self._muoi_progress = 0.0
        self._muoi_present = True
        self._animating_muoi = True
        self._on_flow_complete = on_complete

    def _update_muoi_flow(self, dt: float):
        """Cập nhật vị trí phôi muỗi (gọi từ render loop)"""
        speed = 1.8 * dt  # Normalized speed

        species_color = self.SPECIES_COLORS[self._active_species_id - 1]

        if self._muoi_phase == "RAY":
            self._muoi_progress += speed * 1.3
            # Trail particles
            nx = self.ray_x_start + self._muoi_progress * (self.ray_x_end - self.ray_x_start)
            self._particles.emit_trail(nx, self.ray_y + 10, species_color, count=1, life=0.25)
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "BELT"
                self._muoi_progress = 0.0

        elif self._muoi_phase == "BELT":
            self._muoi_progress += speed
            nx = self.belt_x_start + self._muoi_progress * (self.belt_x_end - self.belt_x_start)
            self._particles.emit_trail(nx, self.belt_y - 20, species_color, count=1, life=0.2)
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "CHUTE"
                self._muoi_progress = 0.0

        elif self._muoi_phase == "CHUTE":
            self._muoi_progress += speed * 2.0
            nx = self.chute_x_start + self._muoi_progress * (self.chute_x_end - self.chute_x_start)
            ny = 145 + self._muoi_progress * 65
            self._particles.emit_trail(nx, ny, species_color, count=1, life=0.3)
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "DISC"
                self._muoi_progress = 1.0
                self._animating_muoi = False

                # Scatter particles khi rơi vào lọ
                self._particles.emit(
                    self.disc_cx - 55, self.disc_cy, species_color,
                    count=12, speed=50.0, life=0.6, size=3.5
                )

                # Muỗi rơi vào lọ tương ứng loài
                incoming_species = self._active_species_id
                target_jar = self.species_assigned_jar.get(incoming_species, 1)
                self.jar_counts[target_jar] += 1

                if self._on_count_change:
                    self._on_count_change(self.jar_counts, self.jar_assigned_species)

                if self._on_flow_complete:
                    self._on_flow_complete()

    def get_active_jar_index(self) -> int:
        """Xác định số thứ tự lọ (1-12) đang ở vị trí máng nạp (tọa độ -90°)"""
        servo_deg = self._current_angle
        jar_idx = (int(round(((-servo_deg) % 360.0) / 30.0)) % 12) + 1
        return jar_idx

    def set_count_change_callback(self, callback):
        self._on_count_change = callback

    # =========================================================================
    # Dark Neon Canvas Rendering (60fps)
    # =========================================================================

    def _redraw(self):
        self.delete("all")
        dt = 0.016  # ~60fps dt for glow calculations

        self._draw_neon_background_panels(dt)
        self._draw_neon_flow_arrows(dt)
        self._draw_stage_1_ray_neon(dt)
        self._draw_stage_2_conveyor_neon(dt)
        self._draw_stage_3_chute_neon(dt)
        self._draw_stage_4_disc_neon_12_jars(dt)

        if self._muoi_present:
            self._draw_moving_muoi_neon()

        # Render particles on top
        self._particles.draw(self)

    def _draw_neon_background_panels(self, dt: float):
        """Vẽ thẻ phân hệ Dark Neon với glow border"""
        panels = [
            (12, 12, 215, 345, "(1) RAY TÁCH MUỖI", 113),
            (220, 12, 515, 345, "(2) BĂNG CHUYỀN TỰ ĐỘNG", 367),
            (520, 12, 585, 345, "(3)", 552),
            (590, 12, 955, 345, "(4) ĐĨA XOAY 12 LỌ 12 LOÀI MUỖI", 772),
        ]

        for x1, y1, x2, y2, title, tx in panels:
            # Shadow layer
            self.create_rectangle(
                x1 + 3, y1 + 3, x2 + 3, y2 + 3,
                fill="#050810", outline="", tags="bg"
            )
            # Main panel
            self.create_rectangle(
                x1, y1, x2, y2,
                fill=NeonTheme.BG_PANEL, outline=NeonTheme.BORDER_DEFAULT, width=1,
                tags="bg"
            )
            # Neon top border accent
            glow_intensity = self._disc_glow.update(dt) * 0.3
            border_color = neon_glow_color(NeonTheme.NEON_CYAN, glow_intensity)
            self.create_line(x1, y1, x2, y1, fill=border_color, width=1.5, tags="bg")

            # Title text
            NeonDraw.neon_text(
                self, tx, 28, title,
                NeonTheme.NEON_CYAN, ("Segoe UI", 9, "bold"),
                glow=True, tags="bg"
            )

    def _draw_neon_flow_arrows(self, dt: float):
        """Mũi tên chuyển động neon giữa các phân hệ"""
        arrows = [
            (210, 180, 225, 180),  # Ray → Belt
            (510, 180, 525, 180),  # Belt → Chute
            (580, 180, 595, 180),  # Chute → Disc
        ]

        phase = (self._idle_glow_phase * 3.0) % 1.0
        for x1, y1, x2, y2 in arrows:
            color = neon_glow_color(NeonTheme.NEON_CYAN, 0.3 + 0.5 * math.sin(phase * math.pi * 2))
            self.create_line(x1, y1, x2, y2, fill=color, width=2.5, arrow="last", arrowshape=(8, 10, 4))
            phase = (phase + 0.33) % 1.0

    def _draw_stage_1_ray_neon(self, dt: float):
        """(1) Phân hệ Ray 3D Neon"""
        ry = self.ray_y

        # Phễu nạp 3D neon
        self.create_rectangle(165, ry - 115, 205, ry - 75, fill=NeonTheme.SURFACE_2, outline=NeonTheme.NEON_CYAN, width=1.5)
        self.create_rectangle(170, ry - 120, 200, ry - 115, fill=NeonTheme.NEON_BLUE, outline=dim_color(NeonTheme.NEON_BLUE, 0.6))
        NeonDraw.neon_text(self, 185, ry - 95, "Phễu Lọ", NeonTheme.NEON_CYAN, ("Segoe UI", 8, "bold"), glow=False)

        # Neon arrow down
        NeonDraw.neon_line(self, 185, ry - 75, 185, ry - 55, NeonTheme.NEON_CYAN, glow_layers=1, width=2)
        self.create_line(185, ry - 58, 185, ry - 52, fill=NeonTheme.NEON_CYAN, width=2, arrow="last")

        # 3 Khay ray nghiêng neon
        for y_offset in [-42, -12, 18]:
            x1, y1 = 30, ry + y_offset
            x2, y2 = 185, ry + y_offset + 18
            # Ray surface
            self.create_polygon(x1, y1, x2, y2, x2, y2 + 6, x1, y1 + 6,
                                fill=NeonTheme.SURFACE_2, outline=NeonTheme.BORDER_DEFAULT)
            # Side depth
            self.create_polygon(x1, y1 + 6, x2, y2 + 6, x2 + 4, y2 + 10, x1 + 4, y1 + 10,
                                fill=NeonTheme.SURFACE_1, outline="")
            # Neon dots on ray
            for dot_x in range(int(x1 + 15), int(x2 - 10), 22):
                dot_y = y1 + (dot_x - x1) * 0.11 + 3
                dot_color = dim_color(NeonTheme.NEON_CYAN, 0.4)
                self.create_oval(dot_x - 1.5, dot_y - 1.5, dot_x + 1.5, dot_y + 1.5,
                                 fill=dot_color, outline="")

    def _draw_stage_2_conveyor_neon(self, dt: float):
        """(2) Băng chuyền 3D Neon với animated chevrons"""
        by = self.belt_y
        bx1, bx2 = self.belt_x_start + 10, self.belt_x_end - 10

        # Body surface
        poly_3d = [bx1, by - 24, bx2, by - 24, bx2 + 12, by + 16, bx1 + 12, by + 16]
        self.create_polygon(poly_3d, fill=NeonTheme.SURFACE_1, outline=dim_color(NeonTheme.NEON_BLUE, 0.4), width=1.5)

        # Top surface
        poly_top = [bx1 + 8, by - 18, bx2 - 8, by - 18, bx2 + 4, by + 10, bx1 + 4, by + 10]
        self.create_polygon(poly_top, fill=NeonTheme.BELT_SURFACE, outline="")

        # Animated chevron stripes
        chevron_spacing = 38
        offset = self._chevron_offset % chevron_spacing
        for x in range(int(bx1 + 20 - offset), int(bx2 - 20), chevron_spacing):
            if x < bx1 + 15:
                continue
            alpha = 0.4 + 0.3 * math.sin((x + self._chevron_offset * 0.5) * 0.1)
            color = dim_color(NeonTheme.NEON_BLUE, alpha)
            self.create_line(x, by - 16, x + 6, by + 8, fill=color, width=2)

        # Neon edge highlights
        self.create_line(bx1 + 8, by - 18, bx2 - 8, by - 18, fill=dim_color(NeonTheme.NEON_BLUE, 0.5), width=1)

        # Sprockets with neon
        for sp_x in [bx1 + 18, bx2 - 18]:
            self._draw_neon_sprocket(sp_x, by, radius=17, teeth=8, angle_deg=self._sprocket_angle)

        # CAMERA AI VISION SENSOR - Animated laser sweep
        cam_x, cam_y = bx1 + 35, by - 75

        # Camera mount
        self.create_line(cam_x, cam_y + 10, cam_x, by - 18, fill=NeonTheme.TEXT_DIM, width=2)

        # Laser sweep cone (animated)
        sweep_phase = math.sin(self._laser_sweep_t * math.pi) * 0.5
        laser_left = cam_x - 14 + sweep_phase * 8
        laser_right = cam_x + 14 + sweep_phase * 8
        laser_intensity = self._scanner_glow.update(dt)
        laser_color = neon_glow_color(NeonTheme.NEON_CYAN, laser_intensity * 0.5)
        laser_fill = dim_color(NeonTheme.NEON_CYAN, 0.08 + laser_intensity * 0.06)

        self.create_polygon(
            laser_left, by - 14, laser_right, by - 14, cam_x, cam_y + 8,
            fill=laser_fill, outline=laser_color, width=1
        )

        # Camera body neon
        self.create_rectangle(cam_x - 14, cam_y - 12, cam_x + 14, cam_y + 10,
                              fill=NeonTheme.SURFACE_1, outline=NeonTheme.NEON_CYAN, width=1.5)
        # Lens with glow
        lens_glow = neon_glow_color(NeonTheme.NEON_CYAN, laser_intensity)
        self.create_oval(cam_x - 6, cam_y - 5, cam_x + 6, cam_y + 5,
                         fill=lens_glow, outline="#ffffff", width=1.5)

        NeonDraw.neon_text(
            self, cam_x, cam_y - 20,
            "📷 AI Vision Sensor",
            NeonTheme.NEON_CYAN, ("Segoe UI", 8, "bold"), glow=True
        )

        # Bottom label
        NeonDraw.neon_text(
            self, (bx1 + bx2) // 2, by + 36,
            "► BĂNG CHUYỀN 3D TỰ ĐỘNG ►",
            NeonTheme.NEON_BLUE, ("Consolas", 9, "bold"), glow=True
        )

    def _draw_neon_sprocket(self, cx: float, cy: float, radius: float, teeth: int, angle_deg: float):
        """Vẽ bánh răng neon glow"""
        points = []
        for i in range(teeth * 2):
            r = radius if i % 2 == 0 else radius * 0.65
            a = math.radians(angle_deg + i * (360 / (teeth * 2)))
            points.extend([cx + r * math.cos(a), cy + r * math.sin(a) * 0.7])

        self.create_polygon(points, fill=NeonTheme.SURFACE_1, outline=NeonTheme.NEON_CYAN, width=1.5)
        # Center hub glow
        self.create_oval(cx - 4, cy - 3, cx + 4, cy + 3, fill=NeonTheme.NEON_CYAN, outline="")

    def _draw_stage_3_chute_neon(self, dt: float):
        """(3) Cần gạt máng dẫn 3D Neon"""
        x1, y1 = 520, 145
        x2, y2 = 585, 210

        # Main chute body
        self.create_polygon(x1, y1, x2, y2, x2 - 8, y2 + 14, x1, y1 + 14,
                            fill=NeonTheme.NEON_BLUE, outline=dim_color(NeonTheme.NEON_BLUE, 0.6), width=1.5)
        # Side depth
        self.create_polygon(x1, y1 + 14, x2 - 8, y2 + 14, x2 - 4, y2 + 18, x1 + 4, y1 + 18,
                            fill=dim_color(NeonTheme.NEON_BLUE, 0.5), outline="")

        NeonDraw.neon_text(self, 552, y1 - 14, "Cần gạt", NeonTheme.NEON_CYAN, ("Segoe UI", 8, "bold"), glow=False)
        NeonDraw.neon_text(self, 552, y2 + 22, "Cấp 1-3", NeonTheme.NEON_BLUE, ("Consolas", 8, "bold"), glow=False)

    def _draw_stage_4_disc_neon_12_jars(self, dt: float):
        """(4) ĐĨA XOAY 3D NEON PERSPECTIVE MANG 12 LỌ"""
        cx, cy = self.disc_cx, self.disc_cy
        rx, ry = self.disc_rx, self.disc_ry

        # --- Neon glow rings around disc ---
        glow_intensity = self._disc_glow.update(dt)
        rim_color = neon_glow_color(NeonTheme.DISC_RIM, glow_intensity * 0.4)

        # Outer glow ring
        for i in range(3, 0, -1):
            expand = i * 4
            gc = dim_color(NeonTheme.NEON_CYAN, 0.05 * (4 - i))
            self.create_oval(cx - rx - expand, cy - ry - expand + 6,
                             cx + rx + expand, cy + ry + expand + 6,
                             fill="", outline=gc, width=1)

        # Shadow beneath disc
        self.create_oval(cx - rx - 4, cy - ry + 14, cx + rx + 4, cy + ry + 18,
                         fill="#050810", outline="")

        # Disc side (3D depth)
        self.create_polygon(
            cx - rx, cy, cx + rx, cy,
            cx + rx, cy + 14, cx - rx, cy + 14,
            fill=NeonTheme.SURFACE_1, outline=dim_color(NeonTheme.NEON_CYAN, 0.25)
        )

        # Main disc surface
        self.create_oval(cx - rx, cy - ry, cx + rx, cy + ry,
                         fill=NeonTheme.DISC_SURFACE, outline=rim_color, width=2)

        # Inner disc ring
        inner_rx, inner_ry = int(rx * 0.78), int(ry * 0.78)
        self.create_oval(cx - inner_rx, cy - inner_ry, cx + inner_rx, cy + inner_ry,
                         fill=NeonTheme.DISC_INNER, outline=dim_color(NeonTheme.NEON_CYAN, 0.3), width=1)

        # Neon arc decorations on disc (rotating)
        arc_phase = self._idle_glow_phase * 30
        for i in range(4):
            arc_start = arc_phase + i * 90
            arc_color = dim_color(NeonTheme.NEON_CYAN, 0.15 + glow_intensity * 0.1)
            self.create_arc(
                cx - inner_rx + 8, cy - inner_ry + 8,
                cx + inner_rx - 8, cy + inner_ry - 8,
                start=arc_start, extent=45, style="arc",
                outline=arc_color, width=1
            )

        # --- Vẽ 12 Lọ 3D (Z-Ordering theo độ sâu Y) ---
        jar_orbit_rx = inner_rx * 0.85
        jar_orbit_ry = inner_ry * 0.85

        servo_deg = self._current_angle
        active_jar_index = self.get_active_jar_index()

        jars_to_draw = []
        for idx in range(12):
            jar_num = idx + 1
            base_deg = idx * 30.0
            total_deg = base_deg + servo_deg - 90

            rad = math.radians(total_deg)
            jx = cx + jar_orbit_rx * math.cos(rad)
            jy = cy + jar_orbit_ry * math.sin(rad)
            jars_to_draw.append((jy, jx, jar_num))

        jars_to_draw.sort(key=lambda item: item[0])

        for jy, jx, jar_num in jars_to_draw:
            is_active = (jar_num == active_jar_index)
            self._draw_neon_jar(jx, jy, jar_num, is_active, dt)

        # Kim chỉ hướng Servo neon
        needle_rad = math.radians(servo_deg - 90)
        nx = cx + (inner_rx * 0.55) * math.cos(needle_rad)
        ny = cy + (inner_ry * 0.55) * math.sin(needle_rad)
        NeonDraw.neon_line(self, cx, cy, nx, ny, NeonTheme.NEON_RED, glow_layers=2, width=2.5)

        # Tâm đĩa neon
        self.create_oval(cx - 6, cy - 4, cx + 6, cy + 4, fill=NeonTheme.NEON_CYAN, outline="#ffffff", width=1.5)

        # Hiển thị nhãn loài Active
        active_species_id = self.jar_assigned_species[active_jar_index]
        active_label = self.SPECIES_NAMES[active_species_id - 1]

        NeonDraw.neon_text(
            self, cx, cy + ry + 26,
            f"SERVO: {servo_deg:.1f}° | LỌ #{active_jar_index}: {active_label}",
            NeonTheme.NEON_CYAN, ("Consolas", 9, "bold"), glow=True
        )

    def _draw_neon_jar(self, jx: float, jy: float, jar_num: int, is_active: bool, dt: float):
        """
        Vẽ 1 Lọ 3D neon glass effect với glow khi active.
        """
        w, h = 22, 28
        species_id = self.jar_assigned_species[jar_num]
        count = self.jar_counts[jar_num]
        species_color = self.SPECIES_COLORS[species_id - 1]

        if is_active:
            glow_val = self._active_jar_glow.update(dt)
            body_fill = dim_color(species_color, 0.12)
            border_color = neon_glow_color(species_color, glow_val)
            border_w = 2.0

            # Active jar glow aura
            for i in range(2, 0, -1):
                aura_color = dim_color(species_color, 0.05 * (3 - i))
                self.create_rectangle(
                    jx - w // 2 - i * 3, jy - h // 2 - i * 3,
                    jx + w // 2 + i * 3, jy + h // 2 + i * 3,
                    fill="", outline=aura_color, width=1
                )
        else:
            body_fill = NeonTheme.SURFACE_2
            border_color = dim_color(species_color, 0.5)
            border_w = 1.2

        # 1. Nắp lọ mang màu loài (neon)
        self.create_oval(jx - 6, jy - h // 2 - 6, jx + 6, jy - h // 2 - 2,
                         fill=species_color, outline=dim_color(species_color, 0.6), width=1)

        # 2. Thân lọ dark glass
        self.create_rectangle(jx - w // 2, jy - h // 2, jx + w // 2, jy + h // 2,
                              fill=body_fill, outline=border_color, width=border_w)

        # Bottom ellipse
        self.create_oval(jx - w // 2, jy + h // 2 - 4, jx + w // 2, jy + h // 2 + 3,
                         fill=NeonTheme.SURFACE_1, outline=border_color, width=border_w)

        # 3. Dải màu loài ở cổ lọ (neon stripe)
        self.create_rectangle(jx - w // 2 + 2, jy - h // 2 + 2, jx + w // 2 - 2, jy - h // 2 + 6,
                              fill=species_color, outline="")

        # 4. SỐ LƯỢNG MUỖI CHÍNH GIỮA LỌ (neon text)
        center_text = f"{count}"
        if is_active:
            text_color = neon_glow_color(NeonTheme.NEON_GREEN, 0.6)
        else:
            text_color = NeonTheme.TEXT_PRIMARY

        # Shadow text
        self.create_text(jx + 1, jy + 2, text=center_text, fill="#000000", font=("Consolas", 10, "bold"))
        self.create_text(jx, jy + 1, text=center_text, fill=text_color, font=("Consolas", 10, "bold"))

        # 5. Nhãn số Lọ
        label_color = species_color if is_active else NeonTheme.TEXT_SECONDARY
        self.create_text(jx, jy - h // 2 - 10, text=f"Lọ{jar_num}", fill=label_color, font=("Segoe UI", 7, "bold"))

    def _draw_moving_muoi_neon(self):
        """Mô phỏng phôi muỗi neon di chuyển qua 4 phân hệ"""
        if self._muoi_phase == "RAY":
            nx = self.ray_x_start + self._muoi_progress * (self.ray_x_end - self.ray_x_start)
            ny = self.ray_y + 10
        elif self._muoi_phase == "BELT":
            nx = self.belt_x_start + self._muoi_progress * (self.belt_x_end - self.belt_x_start)
            ny = self.belt_y - 20
        elif self._muoi_phase == "CHUTE":
            nx = self.chute_x_start + self._muoi_progress * (self.chute_x_end - self.chute_x_start)
            ny = 145 + self._muoi_progress * 65
        else:  # "DISC"
            nx = self.disc_cx - 55
            ny = self.disc_cy

        species_color = self.SPECIES_COLORS[self._active_species_id - 1]
        self._draw_mosquito_neon_icon(nx, ny, species_color)

    def _draw_mosquito_neon_icon(self, cx: float, cy: float, color: str):
        """Vẽ phôi muỗi neon glow"""
        # Outer glow
        glow = dim_color(color, 0.2)
        self.create_oval(cx - 10, cy - 6, cx + 10, cy + 6, fill=glow, outline="")
        # Body
        self.create_oval(cx - 7, cy - 4, cx + 7, cy + 4, fill=color, outline="#ffffff", width=1)
        # Wings neon
        wing_color = neon_glow_color(color, 0.4)
        self.create_line(cx - 3, cy - 4, cx - 9, cy - 9, fill=wing_color, width=1.5)
        self.create_line(cx + 3, cy - 4, cx + 9, cy - 9, fill=wing_color, width=1.5)
