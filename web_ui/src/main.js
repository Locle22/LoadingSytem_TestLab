import './style.css';

const API_BASE = window.location.port !== '3000' 
  ? 'http://localhost:3000/api' 
  : '/api';

// State lưu trữ cục bộ
let speciesList = [];
let isSimulationMode = false;

// DOM Elements
const statusMode = document.getElementById('status-mode');
const statusBackend = document.getElementById('status-backend');
const statusAngle = document.getElementById('status-angle');
const modeToggle = document.getElementById('mode-toggle');
const themeToggle = document.getElementById('theme-toggle');
const modeDescription = document.getElementById('mode-description');
const simulatorPanel = document.getElementById('simulator-panel');

const speciesSelect = document.getElementById('species-select');
const speciesPreview = document.getElementById('species-preview');
const previewName = document.getElementById('preview-name');
const previewId = document.getElementById('preview-id');
const previewColorBox = document.getElementById('preview-color-box');
const previewRgbText = document.getElementById('preview-rgb-text');
const previewSlot = document.getElementById('preview-slot');
const previewAngle = document.getElementById('preview-angle');
const btnSimulate = document.getElementById('btn-simulate');

const btnResetHome = document.getElementById('btn-reset-home');
const rotateLeftDeg = document.getElementById('rotate-left-deg');
const btnRotateLeft = document.getElementById('btn-rotate-left');
const rotateRightDeg = document.getElementById('rotate-right-deg');
const btnRotateRight = document.getElementById('btn-rotate-right');

const slotToAngleSelect = document.getElementById('slot-to-angle-select');
const slotTargetAngle = document.getElementById('slot-target-angle');
const btnSlotToAngle = document.getElementById('btn-slot-to-angle');

const slotFromSelect = document.getElementById('slot-from-select');
const slotToSelect = document.getElementById('slot-to-select');
const btnMoveSlot = document.getElementById('btn-move-slot');

// Active Learning Elements
const btnReconnectCamera = document.getElementById('btn-reconnect-camera');
const btnActiveCapture = document.getElementById('btn-active-capture');
const labelModal = document.getElementById('label-modal');
const btnCloseModal = document.getElementById('btn-close-modal');
const btnCancelCapture = document.getElementById('btn-cancel-capture');
const btnConfirmCapture = document.getElementById('btn-confirm-capture');
const modalPreviewImg = document.getElementById('modal-preview-img');
const modalSpeciesSelect = document.getElementById('modal-species-select');
const modalSlotSelect = document.getElementById('modal-slot-select');
const datasetCount = document.getElementById('dataset-count');
let isCameraPaused = false;

// ──────────────────────────────────────────────
//  Khởi tạo ứng dụng
// ──────────────────────────────────────────────

async function init() {
  const savedTheme = localStorage.getItem('theme') || 'dark';
  document.documentElement.setAttribute('data-theme', savedTheme);
  if (themeToggle) {
    themeToggle.checked = (savedTheme === 'light');
    themeToggle.addEventListener('change', () => {
      const newTheme = themeToggle.checked ? 'light' : 'dark';
      document.documentElement.setAttribute('data-theme', newTheme);
      localStorage.setItem('theme', newTheme);
    });
  }

  await fetchSpeciesList();
  populateSlotDropdowns();
  
  // Lắng nghe sự kiện
  if (btnReconnectCamera) {
    btnReconnectCamera.addEventListener('click', async () => {
      btnReconnectCamera.disabled = true;
      btnReconnectCamera.innerHTML = '⏳ Đang kết nối...';
      await postAPI('/reconnect_camera');
      setTimeout(() => {
        btnReconnectCamera.disabled = false;
        btnReconnectCamera.innerHTML = '🔄 Kết nối lại';
        if (cameraFeed) cameraFeed.src = `${API_BASE}/stream?t=${Date.now()}`;
      }, 2000);
    });
  }
  if (modeToggle) modeToggle.addEventListener('change', handleModeChange);
  speciesSelect.addEventListener('change', handleSpeciesSelectChange);
  btnSimulate.addEventListener('click', handleSimulateClick);
  
  btnResetHome.addEventListener('click', () => postAPI('/reset_home'));
  btnRotateLeft.addEventListener('click', () => {
    const deg = parseInt(rotateLeftDeg.value) || 0;
    postAPI('/rotate_left', { degrees: deg });
  });
  btnRotateRight.addEventListener('click', () => {
    const deg = parseInt(rotateRightDeg.value) || 0;
    postAPI('/rotate_right', { degrees: deg });
  });
  
  btnSlotToAngle.addEventListener('click', () => {
    const slot = slotToAngleSelect.value;
    const angle = parseInt(slotTargetAngle.value) || 0;
    postAPI('/rotate_slot_to_angle', { slot, angle });
  });
  
  btnMoveSlot.addEventListener('click', () => {
    const from = slotFromSelect.value;
    const to = slotToSelect.value;
    postAPI('/move_slot_to_slot', { from, to });
  });

  // Chạy vòng lặp cập nhật trạng thái định kỳ 1s
  setInterval(updateStatus, 1000);
  updateStatus(); // Chạy ngay lần đầu tiên

  refreshCamera(); // Bắt đầu luồng camera
  setInterval(updateDetectionLog, 1000); // Cập nhật log mỗi giây
  updateDetectionLog();
  setInterval(updateDatasetStats, 2000); // Cập nhật thống kê dataset
  updateDatasetStats();
}

// ──────────────────────────────────────────────
//  Gọi các Web API REST
// ──────────────────────────────────────────────

async function postAPI(endpoint, body = {}) {
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    });
    if (!response.ok) {
      console.error(`API Error: ${response.statusText}`);
    }
  } catch (error) {
    console.error(`Failed to post API to ${endpoint}:`, error);
  }
}

async function updateStatus() {
  try {
    const response = await fetch(`${API_BASE}/status`);
    if (!response.ok) return;
    const data = await response.json();

    // Cập nhật góc mâm và tên bộ điều khiển
    statusAngle.textContent = `${data.current_angle}°`;
    statusBackend.textContent = data.backend;

    // Cập nhật chế độ hiển thị
    const backendIsPlc = data.backend.includes('PLC');
    if (backendIsPlc) {
      statusBackend.style.color = '#ef4444';
      statusBackend.style.borderColor = 'rgba(239, 68, 68, 0.3)';
    }

    if (data.mode === 'simulation') {
      isSimulationMode = true;
      statusMode.textContent = '🟡 MÔ PHỎNG';
      statusMode.className = 'badge badge-sim';
      modeToggle.checked = true;
      modeDescription.innerHTML = 'Hệ thống đang ở chế độ <strong>Mô phỏng</strong>. AI camera sẽ tạm ngưng phát lệnh điều khiển mâm, chỉ truyền nhận dữ liệu camera. Mâm sẽ thực thi theo lệnh bấm từ Web.';
      simulatorPanel.classList.remove('disabled');
      speciesSelect.removeAttribute('disabled');
      btnSimulate.removeAttribute('disabled');
    } else {
      isSimulationMode = false;
      statusMode.textContent = '🟢 TỰ ĐỘNG';
      statusMode.className = 'badge badge-auto';
      modeToggle.checked = false;
      modeDescription.innerHTML = 'Hệ thống đang ở chế độ <strong>Tự động</strong>. Luồng AI camera sẽ liên tục gửi lệnh phân loại xuống phần cứng.';
      simulatorPanel.classList.add('disabled');
      speciesSelect.setAttribute('disabled', 'true');
      btnSimulate.setAttribute('disabled', 'true');
    }
  } catch (error) {
    console.warn('Backend API Server is not running yet. Status checking retrying...');
  }
}

async function fetchSpeciesList() {
  try {
    const response = await fetch(`${API_BASE}/species_list`);
    if (response.ok) {
      speciesList = await response.json();
      
      // Populate select dropdown
      speciesSelect.innerHTML = '<option value="">-- Chọn một loài muỗi --</option>';
      if (modalSpeciesSelect) {
        modalSpeciesSelect.innerHTML = '<option value="">-- Chọn loài muỗi đúng --</option><option value="new">➕ [Loài mới] Nhập tên loài muỗi khác...</option>';
      }
      speciesList.forEach(sp => {
        const opt = document.createElement('option');
        opt.value = sp.class_id;
        opt.textContent = `[ID ${sp.class_id}] ${sp.name}`;
        speciesSelect.appendChild(opt);

        if (modalSpeciesSelect) {
          const optModal = document.createElement('option');
          optModal.value = sp.class_id;
          optModal.textContent = `[ID ${sp.class_id}] ${sp.name} (${sp.slot})`;
          modalSpeciesSelect.appendChild(optModal);
        }
      });
    } else {
      speciesSelect.innerHTML = `<option value="">-- Lỗi tải (HTTP ${response.status}) --</option>`;
    }
  } catch (error) {
    console.error('Failed to fetch species list:', error);
    speciesSelect.innerHTML = `<option value="">-- Lỗi kết nối: ${error.message} --</option>`;
  }
}

function populateSlotDropdowns() {
  // Tạo danh sách 36 ô A-Z, A0-J0
  const slots = [];
  
  // A..Z (0..25)
  for (let i = 0; i < 26; i++) {
    slots.push(String.fromCharCode(65 + i));
  }
  // A0..J0 (26..35)
  for (let i = 0; i < 10; i++) {
    slots.push(String.fromCharCode(65 + i) + '0');
  }

  // Đổ dữ liệu vào các select box
  const selects = [slotToAngleSelect, slotFromSelect, slotToSelect, modalSlotSelect].filter(Boolean);
  selects.forEach(select => {
    select.innerHTML = '';
    slots.forEach(slot => {
      const opt = document.createElement('option');
      opt.value = slot;
      opt.textContent = `Ô ${slot}`;
      select.appendChild(opt);
    });
  });
  
  // Đặt giá trị mặc định khác nhau cho From/To để dễ nhìn
  if (slotToSelect.children.length > 1) {
    slotToSelect.selectedIndex = 1; // Ô B
  }
}

// ──────────────────────────────────────────────
//  Xử lý sự kiện UI
// ──────────────────────────────────────────────

function handleModeChange() {
  if (modeToggle.checked) {
    postAPI('/mode/simulation');
  } else {
    postAPI('/mode/auto');
  }
}

function handleSpeciesSelectChange() {
  const classId = speciesSelect.value;
  if (classId === '') {
    speciesPreview.classList.add('hidden');
    return;
  }

  const sp = speciesList.find(s => s.class_id == classId);
  if (sp) {
    previewName.textContent = sp.name;
    previewId.textContent = sp.class_id;
    
    // Mảng RGB
    const [r, g, b] = sp.rgb;
    previewColorBox.style.backgroundColor = `rgb(${r}, ${g}, ${b})`;
    previewColorBox.style.color = `rgb(${r}, ${g}, ${b})`;
    previewRgbText.textContent = `RGB(${r}, ${g}, ${b})`;
    
    previewSlot.textContent = `Ô ${sp.slot}`;
    previewAngle.textContent = `${sp.angle}°`;
    
    speciesPreview.classList.remove('hidden');
  }
}

function handleSimulateClick() {
  const classId = speciesSelect.value;
  if (classId !== '') {
    postAPI('/simulate_mosquito', { class_id: parseInt(classId) });
  }
}

// ──────────────────────────────────────────────
//  Camera Feed (Snapshot Polling)
// ──────────────────────────────────────────────

const cameraFeed = document.getElementById('camera-feed');
const cameraOverlay = document.getElementById('camera-overlay');
const cameraStatus = document.getElementById('camera-status');
let cameraConnected = false;
let failedFetchCount = 0;

function refreshCamera() {
  if (!cameraFeed.src || !cameraFeed.src.includes('/stream')) {
    cameraFeed.src = `${API_BASE}/stream?t=${Date.now()}`;
  }
  
  fetch(`${API_BASE}/status`)
    .then(res => {
      failedFetchCount = 0;
      if (!cameraConnected) {
        cameraConnected = true;
        cameraOverlay.classList.add('hidden');
        cameraStatus.textContent = '🟢 Trực tiếp (30 FPS)';
        cameraStatus.classList.add('connected');
      }
    })
    .catch(() => {
      failedFetchCount++;
      if (failedFetchCount >= 3 && cameraConnected) {
        cameraConnected = false;
        cameraOverlay.classList.remove('hidden');
        cameraStatus.textContent = '🔴 Mất kết nối';
        cameraStatus.classList.remove('connected');
      }
    })
    .finally(() => {
      setTimeout(refreshCamera, 1000); // Kiểm tra nhịp tim kết nối mỗi 1 giây
    });
}

// ──────────────────────────────────────────────
//  Detection Log (Real-time History)
// ──────────────────────────────────────────────

const logBody = document.getElementById('log-body');
const logCount = document.getElementById('log-count');
let lastLogLength = 0;

async function updateDetectionLog() {
  try {
    const response = await fetch(`${API_BASE}/detection_log`);
    if (!response.ok) return;
    const entries = await response.json();

    if (entries.length === lastLogLength) return; // No change
    lastLogLength = entries.length;
    logCount.textContent = `${entries.length} bản ghi`;

    if (entries.length === 0) {
      logBody.innerHTML = '<tr class="log-empty"><td colspan="7">Chưa có dữ liệu nhận diện...</td></tr>';
      return;
    }

    logBody.innerHTML = entries.map(e => {
      const time = new Date(e.timestamp_ms).toLocaleTimeString('vi-VN');
      const [r, g, b] = e.rgb;
      const conf = (e.confidence * 100).toFixed(1);
      return `<tr class="log-row">
        <td class="log-time">${time}</td>
        <td class="log-species">${e.species_name}</td>
        <td>${e.class_id}</td>
        <td><span class="conf-badge">${conf}%</span></td>
        <td><span class="color-dot" style="background:rgb(${r},${g},${b})"></span> RGB(${r},${g},${b})</td>
        <td class="log-slot">Ô ${e.slot}</td>
        <td>${e.angle}°</td>
      </tr>`;
    }).join('');
  } catch (err) {
    // API not available yet
  }
}

// ──────────────────────────────────────────────
//  Active Learning & Dataset Management
// ──────────────────────────────────────────────

if (btnActiveCapture) {
  btnActiveCapture.addEventListener('click', () => {
    isCameraPaused = true; // Dừng cập nhật trạng thái UI
    postAPI('/freeze_sample_frame'); // Khóa frame hiện tại trên backend!
    if (modalPreviewImg) {
      modalPreviewImg.src = `${API_BASE}/snapshot?t=${Date.now()}`;
    }
    if (modalSpeciesSelect) modalSpeciesSelect.value = '';
    const customGroup = document.getElementById('custom-species-group');
    if (customGroup) customGroup.classList.add('hidden');
    const customInput = document.getElementById('modal-custom-species');
    if (customInput) customInput.value = '';

    if (labelModal) labelModal.classList.remove('hidden');
  });
}

function closeLabelModal() {
  if (labelModal) labelModal.classList.add('hidden');
  isCameraPaused = false; // Tiếp tục luồng live camera
  postAPI('/unfreeze_sample_frame'); // Hủy khóa frame trên backend
}

if (btnCloseModal) btnCloseModal.addEventListener('click', closeLabelModal);
if (btnCancelCapture) btnCancelCapture.addEventListener('click', closeLabelModal);

if (modalSpeciesSelect) {
  modalSpeciesSelect.addEventListener('change', () => {
    const classId = modalSpeciesSelect.value;
    const customGroup = document.getElementById('custom-species-group');
    if (classId === 'new') {
      if (customGroup) customGroup.classList.remove('hidden');
      const customInput = document.getElementById('modal-custom-species');
      if (customInput) customInput.focus();
    } else {
      if (customGroup) customGroup.classList.add('hidden');
      if (classId !== '') {
        const sp = speciesList.find(s => s.class_id == classId);
        if (sp && modalSlotSelect) {
          modalSlotSelect.value = sp.slot;
        }
      }
    }
  });
}

if (btnConfirmCapture) {
  btnConfirmCapture.addEventListener('click', async () => {
    const classId = modalSpeciesSelect.value;
    const slot = modalSlotSelect.value;
    if (classId === '') {
      alert('Vui lòng chọn tên loài muỗi hoặc chọn nhập loài mới!');
      return;
    }

    let speciesName = '';
    let angle = 0;
    let finalClassId = 0;

    if (classId === 'new') {
      const customInput = document.getElementById('modal-custom-species');
      speciesName = customInput ? customInput.value.trim() : '';
      if (!speciesName) {
        alert('Vui lòng nhập tên khoa học cho loài muỗi mới!');
        if (customInput) customInput.focus();
        return;
      }
      finalClassId = speciesList.length > 0 ? Math.max(...speciesList.map(s => s.class_id)) + 1 : 36;
      const slotIdx = slot.charCodeAt(0) - 65;
      angle = (slotIdx >= 0 && slotIdx < 36) ? slotIdx * 10 : 0;
    } else {
      finalClassId = parseInt(classId);
      const sp = speciesList.find(s => s.class_id == finalClassId);
      speciesName = sp ? sp.name : `Class ${finalClassId}`;
      angle = sp ? sp.angle : 0;
    }

    await postAPI('/capture_sample', {
      class_id: finalClassId,
      species_name: speciesName,
      slot: slot,
      angle: angle
    });

    closeLabelModal();
    updateDatasetStats();
  });
}

async function updateDatasetStats() {
  try {
    const res = await fetch(`${API_BASE}/dataset/stats`);
    if (res.ok) {
      const data = await res.json();
      if (datasetCount) {
        datasetCount.textContent = `${data.total_samples} mẫu`;
      }
    }
  } catch (e) {
    // ignore
  }
}

// Chạy khởi tạo khi trang load xong
window.addEventListener('DOMContentLoaded', init);
