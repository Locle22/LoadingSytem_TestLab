# =============================================================================
# LoadingSystem HMIv2 - Animation Engine (60fps Neon Effects)
# =============================================================================
# Module quản lý animation 60fps:
#   - Easing functions (cubic, elastic, bounce)
#   - Neon glow rendering utilities cho tkinter Canvas
#   - Particle system cho trail effects
#   - Pulse glow controller (sin wave intensity modulation)
# =============================================================================

import math
import time
import random
from typing import List, Tuple, Optional, Callable


# =============================================================================
# Easing Functions
# =============================================================================

def ease_linear(t: float) -> float:
    return t

def ease_in_out_cubic(t: float) -> float:
    if t < 0.5:
        return 4.0 * t * t * t
    else:
        p = 2.0 * t - 2.0
        return 0.5 * p * p * p + 1.0

def ease_out_cubic(t: float) -> float:
    p = t - 1.0
    return p * p * p + 1.0

def ease_in_cubic(t: float) -> float:
    return t * t * t

def ease_out_elastic(t: float) -> float:
    if t == 0 or t == 1:
        return t
    p = 0.3
    s = p / 4.0
    return math.pow(2, -10 * t) * math.sin((t - s) * (2 * math.pi) / p) + 1.0

def ease_out_bounce(t: float) -> float:
    if t < 1 / 2.75:
        return 7.5625 * t * t
    elif t < 2 / 2.75:
        t -= 1.5 / 2.75
        return 7.5625 * t * t + 0.75
    elif t < 2.5 / 2.75:
        t -= 2.25 / 2.75
        return 7.5625 * t * t + 0.9375
    else:
        t -= 2.625 / 2.75
        return 7.5625 * t * t + 0.984375


# =============================================================================
# Color Utilities
# =============================================================================

def hex_to_rgb(hex_color: str) -> Tuple[int, int, int]:
    """Convert hex color string to RGB tuple"""
    hex_color = hex_color.lstrip('#')
    return tuple(int(hex_color[i:i+2], 16) for i in (0, 2, 4))


def rgb_to_hex(r: int, g: int, b: int) -> str:
    """Convert RGB to hex color string"""
    r = max(0, min(255, int(r)))
    g = max(0, min(255, int(g)))
    b = max(0, min(255, int(b)))
    return f"#{r:02x}{g:02x}{b:02x}"


def lerp_color(c1: str, c2: str, t: float) -> str:
    """Linearly interpolate between two hex colors"""
    r1, g1, b1 = hex_to_rgb(c1)
    r2, g2, b2 = hex_to_rgb(c2)
    r = r1 + (r2 - r1) * t
    g = g1 + (g2 - g1) * t
    b = b1 + (b2 - b1) * t
    return rgb_to_hex(int(r), int(g), int(b))


def neon_glow_color(base_color: str, intensity: float) -> str:
    """
    Tạo màu neon glow bằng cách blend base_color với trắng theo intensity.
    intensity: 0.0 (gốc) → 1.0 (sáng nhất)
    """
    r, g, b = hex_to_rgb(base_color)
    glow_r = int(r + (255 - r) * intensity * 0.6)
    glow_g = int(g + (255 - g) * intensity * 0.6)
    glow_b = int(b + (255 - b) * intensity * 0.6)
    return rgb_to_hex(glow_r, glow_g, glow_b)


def dim_color(base_color: str, factor: float) -> str:
    """Làm tối màu theo factor (0.0 = đen, 1.0 = giữ nguyên)"""
    r, g, b = hex_to_rgb(base_color)
    return rgb_to_hex(int(r * factor), int(g * factor), int(b * factor))


# =============================================================================
# Pulse Glow Controller
# =============================================================================

class PulseGlow:
    """
    Controller cho hiệu ứng pulse glow (sáng tối theo sin wave).
    Dùng cho neon border, active jar highlight, connection indicator.
    """

    def __init__(self, speed: float = 2.0, min_val: float = 0.3, max_val: float = 1.0):
        """
        Args:
            speed: Tốc độ nhấp nháy (Hz)
            min_val: Giá trị intensity tối thiểu
            max_val: Giá trị intensity tối đa
        """
        self.speed = speed
        self.min_val = min_val
        self.max_val = max_val
        self._phase = 0.0

    def update(self, dt: float) -> float:
        """Cập nhật và trả về intensity hiện tại (0.0-1.0)"""
        self._phase += dt * self.speed * 2 * math.pi
        if self._phase > 2 * math.pi:
            self._phase -= 2 * math.pi
        t = (math.sin(self._phase) + 1.0) / 2.0
        return self.min_val + (self.max_val - self.min_val) * t

    def get_color(self, base_color: str, dt: float) -> str:
        """Trả về màu neon glow tại thời điểm hiện tại"""
        intensity = self.update(dt)
        return neon_glow_color(base_color, intensity)


# =============================================================================
# Particle System (Lightweight)
# =============================================================================

class Particle:
    """Hạt particle đơn lẻ cho trail/scatter effects"""
    __slots__ = ['x', 'y', 'vx', 'vy', 'life', 'max_life', 'size', 'color']

    def __init__(self, x: float, y: float, vx: float, vy: float,
                 life: float, size: float, color: str):
        self.x = x
        self.y = y
        self.vx = vx
        self.vy = vy
        self.life = life
        self.max_life = life
        self.size = size
        self.color = color


class ParticleSystem:
    """
    Hệ thống hạt particle cho neon trail và scatter effects.
    Tối ưu cho 60fps — giới hạn tối đa 80 hạt.
    """
    MAX_PARTICLES = 80

    def __init__(self):
        self.particles: List[Particle] = []

    def emit(self, x: float, y: float, color: str,
             count: int = 3, speed: float = 30.0, life: float = 0.5,
             size: float = 3.0, spread: float = math.pi * 2):
        """Phát hạt particle tại vị trí (x, y)"""
        for _ in range(count):
            angle = random.uniform(0, spread)
            spd = random.uniform(speed * 0.3, speed)
            vx = spd * math.cos(angle)
            vy = spd * math.sin(angle)
            p_life = random.uniform(life * 0.5, life)
            p_size = random.uniform(size * 0.5, size)

            if len(self.particles) < self.MAX_PARTICLES:
                self.particles.append(Particle(x, y, vx, vy, p_life, p_size, color))

    def emit_trail(self, x: float, y: float, color: str,
                   count: int = 2, life: float = 0.3, size: float = 2.5):
        """Phát hạt trail (rơi xuống dưới nhẹ)"""
        for _ in range(count):
            vx = random.uniform(-8, 8)
            vy = random.uniform(-15, 5)
            p_life = random.uniform(life * 0.4, life)
            p_size = random.uniform(size * 0.4, size)

            if len(self.particles) < self.MAX_PARTICLES:
                self.particles.append(Particle(x, y, vx, vy, p_life, p_size, color))

    def update(self, dt: float):
        """Cập nhật tất cả hạt particle"""
        alive = []
        for p in self.particles:
            p.life -= dt
            if p.life > 0:
                p.x += p.vx * dt
                p.y += p.vy * dt
                p.vy += 40.0 * dt  # Gravity nhẹ
                alive.append(p)
        self.particles = alive

    def draw(self, canvas):
        """Render tất cả hạt particle lên canvas"""
        for p in self.particles:
            alpha = max(0.0, p.life / p.max_life)
            size = p.size * alpha
            if size < 0.5:
                continue

            # Tạo màu mờ dần theo lifetime
            color = dim_color(p.color, alpha)
            canvas.create_oval(
                p.x - size, p.y - size,
                p.x + size, p.y + size,
                fill=color, outline="", tags="particle"
            )

    def clear(self):
        self.particles.clear()


# =============================================================================
# Neon Canvas Drawing Utilities
# =============================================================================

class NeonDraw:
    """
    Bộ công cụ vẽ neon glow trên tkinter Canvas.
    Mỗi neon element = nhiều layer với opacity giảm dần ra ngoài.
    """

    @staticmethod
    def neon_oval(canvas, x1, y1, x2, y2, color: str, glow_layers: int = 3,
                  fill: str = "", width: float = 2.0, tags: str = ""):
        """Vẽ oval với neon glow effect"""
        for i in range(glow_layers, 0, -1):
            expand = i * 2.5
            glow_color = dim_color(color, 0.15 + 0.2 * (glow_layers - i) / glow_layers)
            canvas.create_oval(
                x1 - expand, y1 - expand, x2 + expand, y2 + expand,
                outline=glow_color, width=width + i * 1.5, fill="",
                tags=tags
            )
        canvas.create_oval(x1, y1, x2, y2, outline=color, width=width, fill=fill, tags=tags)

    @staticmethod
    def neon_line(canvas, x1, y1, x2, y2, color: str, glow_layers: int = 2,
                  width: float = 2.0, tags: str = "", **kwargs):
        """Vẽ line với neon glow effect"""
        for i in range(glow_layers, 0, -1):
            glow_color = dim_color(color, 0.12 + 0.2 * (glow_layers - i) / glow_layers)
            canvas.create_line(
                x1, y1, x2, y2,
                fill=glow_color, width=width + i * 2.5,
                tags=tags, **kwargs
            )
        canvas.create_line(x1, y1, x2, y2, fill=color, width=width, tags=tags, **kwargs)

    @staticmethod
    def neon_rect(canvas, x1, y1, x2, y2, color: str, glow_layers: int = 2,
                  fill: str = "", width: float = 1.5, tags: str = ""):
        """Vẽ rectangle với neon glow border"""
        for i in range(glow_layers, 0, -1):
            expand = i * 2
            glow_color = dim_color(color, 0.12 + 0.15 * (glow_layers - i) / glow_layers)
            canvas.create_rectangle(
                x1 - expand, y1 - expand, x2 + expand, y2 + expand,
                outline=glow_color, width=width + i, fill="",
                tags=tags
            )
        canvas.create_rectangle(x1, y1, x2, y2, outline=color, width=width, fill=fill, tags=tags)

    @staticmethod
    def neon_text(canvas, x, y, text: str, color: str, font: tuple,
                  glow: bool = True, tags: str = ""):
        """Vẽ text với neon glow shadow"""
        if glow:
            glow_color = dim_color(color, 0.3)
            for dx, dy in [(1, 1), (-1, -1), (1, -1), (-1, 1), (0, 2), (2, 0)]:
                canvas.create_text(x + dx, y + dy, text=text, fill=glow_color, font=font, tags=tags)
        canvas.create_text(x, y, text=text, fill=color, font=font, tags=tags)

    @staticmethod
    def neon_arc(canvas, x1, y1, x2, y2, start: float, extent: float,
                 color: str, width: float = 2.0, glow_layers: int = 2, tags: str = ""):
        """Vẽ arc với neon glow"""
        for i in range(glow_layers, 0, -1):
            expand = i * 2
            glow_color = dim_color(color, 0.15 + 0.2 * (glow_layers - i) / glow_layers)
            canvas.create_arc(
                x1 - expand, y1 - expand, x2 + expand, y2 + expand,
                start=start, extent=extent, style="arc",
                outline=glow_color, width=width + i * 2,
                tags=tags
            )
        canvas.create_arc(
            x1, y1, x2, y2, start=start, extent=extent, style="arc",
            outline=color, width=width, tags=tags
        )


# =============================================================================
# Animated Value (Tween)
# =============================================================================

class AnimatedValue:
    """
    Giá trị có animation tween với easing function.
    Dùng cho mọi property cần animate (angle, position, opacity, ...).
    """

    def __init__(self, initial: float = 0.0):
        self.current = initial
        self.start_val = initial
        self.end_val = initial
        self.duration = 0.0
        self.elapsed = 0.0
        self.easing: Callable[[float], float] = ease_linear
        self._animating = False

    @property
    def is_animating(self) -> bool:
        return self._animating

    def animate_to(self, target: float, duration: float = 0.5,
                   easing: Callable[[float], float] = ease_in_out_cubic):
        """Bắt đầu animation tới giá trị target"""
        if abs(target - self.current) < 0.001:
            self.current = target
            self._animating = False
            return

        self.start_val = self.current
        self.end_val = target
        self.duration = max(0.016, duration)
        self.elapsed = 0.0
        self.easing = easing
        self._animating = True

    def set_immediate(self, value: float):
        """Set giá trị ngay lập tức, cancel animation"""
        self.current = value
        self.start_val = value
        self.end_val = value
        self._animating = False

    def update(self, dt: float) -> float:
        """Cập nhật animation theo delta time, trả về giá trị hiện tại"""
        if not self._animating:
            return self.current

        self.elapsed += dt
        t = min(1.0, self.elapsed / self.duration)
        eased_t = self.easing(t)
        self.current = self.start_val + (self.end_val - self.start_val) * eased_t

        if t >= 1.0:
            self.current = self.end_val
            self._animating = False

        return self.current


# =============================================================================
# HMIv2 Neon Color Theme Constants
# =============================================================================

class NeonTheme:
    """Bảng màu Dark Neon cho HMIv2"""

    # Backgrounds
    BG_DEEP = "#0a0e1a"
    BG_PANEL = "#0d1321"
    BG_CARD = "#111827"
    BG_INPUT = "#0c1220"
    BG_CANVAS = "#080c16"

    # Neon Accents
    NEON_CYAN = "#00e5ff"
    NEON_GREEN = "#39ff14"
    NEON_RED = "#ff073a"
    NEON_YELLOW = "#ffea00"
    NEON_PURPLE = "#bf00ff"
    NEON_ORANGE = "#ff6d00"
    NEON_BLUE = "#2979ff"
    NEON_PINK = "#ff4081"

    # Surface Colors
    SURFACE_1 = "#141b2d"
    SURFACE_2 = "#1a2332"
    SURFACE_3 = "#1e293b"

    # Text
    TEXT_PRIMARY = "#e2e8f0"
    TEXT_SECONDARY = "#94a3b8"
    TEXT_DIM = "#64748b"

    # Borders
    BORDER_DEFAULT = "#1e293b"
    BORDER_NEON = "#00e5ff"

    # Header / Status Bar
    HEADER_BG = "#060a14"
    STATUS_BG = "#060a14"

    # Disc specific
    DISC_SURFACE = "#1a2744"
    DISC_RIM = "#00e5ff"
    DISC_INNER = "#0f1b30"

    # Conveyor
    BELT_SURFACE = "#1565c0"
    BELT_TOP = "#1976d2"

    # Log Console
    LOG_BG = "#050810"
    LOG_BORDER = "#00e5ff"

    # 12 Species Neon Colors (Brighter cho dark background)
    SPECIES_COLORS = [
        "#ff1744",  # 1. Aedes aegypti - Neon Red
        "#2979ff",  # 2. Anopheles - Neon Blue
        "#00e676",  # 3. Culex quinque - Neon Green
        "#d500f9",  # 4. Mansonia - Neon Purple
        "#ff9100",  # 5. Aedes albopictus - Neon Orange
        "#00e5ff",  # 6. Anopheles minimus - Neon Cyan
        "#ff4081",  # 7. Culex tritaenio - Neon Pink
        "#ffea00",  # 8. Armigeres - Neon Yellow
        "#69f0ae",  # 9. Coquillettidia - Neon Mint
        "#7c4dff",  # 10. Toxorhynchites - Neon Indigo
        "#76ff03",  # 11. Culiseta - Neon Lime
        "#f50057",  # 12. Psorophora - Neon Rose
    ]

    # Log Tag Colors (Neon)
    LOG_TAGS = {
        "TIME": "#546e7a",
        "INFO": "#00e5ff",
        "SYSTEM": "#00e5ff",
        "DISPENSE": "#39ff14",
        "ROTATE": "#ffea00",
        "ERROR": "#ff073a",
        "CONFIG": "#bf00ff",
    }
