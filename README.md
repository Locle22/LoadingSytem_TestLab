******************************************************************BẢN DEMO RUST V1**************************************************************
Giao tiếp giữa raspberry và esp32 s3, đổi màu đèn RGB trên ESP32 dựa trên từng loại muỗi được random, random delay để tạo độ trễ ngẫu nhiên mô phỏng 
muỗi chạy trên băng chuyền đi qua camera, hardcode Mô hình AI ( sử dụng hàm random loại muỗi và confident trên 1 mảng N phần từ khai báo các loại muỗi)
- Số loại muỗi N = 10
- Số lọ phân loại N' = N + 1 ( Lọ N +1 để chứa muỗi có confident thấp hơn ngưỡng UNKNOW = 30%)
  
