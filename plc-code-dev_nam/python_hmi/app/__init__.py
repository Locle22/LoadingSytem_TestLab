# =============================================================================
# LoadingSystem - Python HMI Application Package
# =============================================================================

"""
LoadingSystem HMI - Giao diện điều khiển đĩa xoay.

Modules:
    hmi_app: Main application window
    zmq_client: IPC client giao tiếp với Rust Backend
    disc_visualizer: Canvas vẽ đĩa xoay realtime
    control_panel: Bảng điều khiển chính
    override_panel: Panel Level 4-5 Manual Override
"""

# Hằng số chia sẻ
ENCODER_RESOLUTION = 18000    # Độ phân giải định mức cho PLC DVP-14SS2 (1 vòng = 18000 xung, Ft-3.06)
DEFAULT_FREQUENCY = 9_000     # Hz (Tần số an toàn < 10kHz)


def degrees_to_pulses(degrees: float, resolution: int = ENCODER_RESOLUTION) -> int:
    """
    Quy đổi góc (độ) sang số xung.
    Công thức: P = round(θ × resolution / 360)

    Args:
        degrees: Góc cần quy đổi
        resolution: Độ phân giải encoder (mặc định 131072)

    Returns:
        Số xung (integer)
    """
    return round(degrees * resolution / 360.0)


def pulses_to_degrees(pulses: int, resolution: int = ENCODER_RESOLUTION) -> float:
    """
    Quy đổi số xung ngược lại thành góc (độ).

    Args:
        pulses: Số xung
        resolution: Độ phân giải encoder

    Returns:
        Góc tính bằng độ
    """
    return pulses * 360.0 / resolution
