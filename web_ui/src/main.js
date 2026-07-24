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

// ──────────────────────────────────────────────
//  Khởi tạo ứng dụng
// ──────────────────────────────────────────────

async function init() {
  await fetchSpeciesList();
  populateSlotDropdowns();
  
  // Lắng nghe sự kiện
  modeToggle.addEventListener('change', handleModeChange);
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
      speciesList.forEach(sp => {
        const opt = document.createElement('option');
        opt.value = sp.class_id;
        opt.textContent = `[ID ${sp.class_id}] ${sp.name}`;
        speciesSelect.appendChild(opt);
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
  const selects = [slotToAngleSelect, slotFromSelect, slotToSelect];
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
  const newImg = new Image();
  newImg.onload = function() {
    failedFetchCount = 0;
    cameraFeed.src = this.src;
    if (!cameraConnected) {
      cameraConnected = true;
      cameraOverlay.classList.add('hidden');
      cameraStatus.textContent = '🟢 Trực tiếp';
      cameraStatus.classList.add('connected');
    }
    setTimeout(refreshCamera, 100); // ~10 FPS
  };
  newImg.onerror = function() {
    failedFetchCount++;
    if (failedFetchCount >= 3 && cameraConnected) {
      cameraConnected = false;
      cameraOverlay.classList.remove('hidden');
      cameraStatus.textContent = '🔴 Mất kết nối';
      cameraStatus.classList.remove('connected');
    }
    setTimeout(refreshCamera, 300); // Thử lại nhanh sau 300ms
  };
  newImg.src = `${API_BASE}/snapshot?t=${Date.now()}`;
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

// Chạy khởi tạo khi trang load xong
window.addEventListener('DOMContentLoaded', init);
