# Ghi nhận Vấn đề & Giải pháp (Version 1.2)

1. **Lỗi:** Khi thao tác lệnh xoay trái/phải với góc bất kỳ, thao tác 2 lệnh quay giống nhau liên tiếp bất kỳ (vd: xoay phải 30 độ) thì 2 lần quay liên tiếp đưa ra góc quay khác nhau trên motor ngoài thực tế.
   - **Tình trạng:** Đã khắc phục [DONE]
   - **Giải pháp:** Cập nhật cơ chế nhận biết cờ hoàn thành phát xung (đọc bit M1029 thay vì M0 trên PLC Delta DVP-14SS2) để tránh việc ghi đè lệnh khi xung chưa phát xong.

2. **Lỗi:** HMI xoay quá nhanh, không đồng nhất với motor ngoài thực tế. Đồng ý có thể có lệch thời gian hoặc sai số, nhưng phải tối thiểu thời gian lệch và bắt buộc mô phỏng quay phải giống với hướng quay ngoài thực tế.
   - **Tình trạng:** Đã khắc phục [DONE]
   - **Giải pháp:** Áp dụng hệ số bù trễ (1.4x latency factor) cho thời gian animation trên HMI, đồng thời đồng bộ hướng mô phỏng và cập nhật thuật toán tự động tính toán tần số phát xung để khớp nhịp giữa phần mềm và Servo Driver.

3. **Lỗi:** Chúng ta đang mô phỏng thả muỗi quá nhanh, nên nhiều khi motor không quay kịp hoặc bị xung đột xung truyền từ PLC, cần mô phỏng chậm lại, thời gian có thể dài nhưng bắt buộc phải hoạt động đúng với % chính xác cao nhất. Thêm vào đó, motor gặp hiện tượng văng trượt quán tính làm sai lệch khoảng 10 độ khi dừng.
   - **Tình trạng:** Đã khắc phục [DONE]
   - **Giải pháp:** 
     - Giảm tốc độ quay giới hạn xuống **0.5 vòng/giây (9,000 Hz)** để triệt tiêu lực quán tính cơ khí.
     - Tăng độ phân giải điều khiển lên gấp 5 lần: **18,000 xung/vòng** (0.02 độ/xung) để phần mềm PLC và Motor Servo CSD7 đồng bộ tuyệt đối với sai số gần như bằng không.
     - Cấu hình lại Electronic Gear trên phần cứng CSD7: Tử số (Ft-3.05) = 131072, Mẫu số (Ft-3.06) = 18000.
