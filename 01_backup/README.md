# VCI BIM Backup Tool

Công cụ backup dữ liệu từ **`\\Vci-bim-nas\projects`** sang ổ cứng rời **Elements**,
viết bằng **Rust** (giao diện **egui/eframe**), chạy đa luồng, tối ưu RAM và tốc độ.

---

## 1. Vì sao chọn Rust?

- Biên dịch thành 1 file `.exe` duy nhất, chạy trực tiếp trên Windows, **không cần cài
  thêm gì** (không cần Python, không cần .NET runtime).
- Không có garbage collector → RAM ổn định, thấp, không có hiện tượng "giật" khi copy
  hàng nghìn file.
- Copy file bằng buffer cố định 4MB tái sử dụng cho từng luồng (xem `src/copier.rs`)
  → RAM chỉ tốn khoảng `(số luồng × 4MB)`, không phụ thuộc dung lượng file (an toàn kể
  cả với file Revit/AutoCAD nặng hàng chục GB).
- Đa luồng thật (OS threads) bằng thư viện chuẩn của Rust, không bị giới hạn bởi GIL
  như Python.

## 2. Yêu cầu để build

Máy bạn đã có sẵn Rust toolchain (thấy thư mục `.cargo`, `.rustup` trong home directory),
nên chỉ cần mở terminal tại thư mục này và chạy:

```bash
cargo build --release
```

File thực thi sẽ nằm tại: `target\release\vci_backup.exe`

Copy file `.exe` này và file `config.toml` ra một thư mục riêng (ví dụ Desktop) để
dùng hàng ngày — không cần mang theo mã nguồn hay `target\`.

> Lần build đầu tiên sẽ tải một số thư viện qua mạng và mất vài phút. Các lần sau sẽ
> nhanh hơn nhiều nhờ cache.

Muốn chạy thử nhanh không cần build file `.exe` riêng:

```bash
cargo run --release
```

## 3. Cấu hình (`config.toml`)

File `config.toml` nằm cạnh `vci_backup.exe`. Mở bằng Notepad để chỉnh, hoặc chỉnh
ngay trong giao diện rồi bấm **"Lưu cấu hình"**.

```toml
source_root = "\\\\Vci-bim-nas\\projects"

projects = [
    "Project_A",
    "Project_B",
]

default_destination = ""
thread_count = 0
enable_mirror_delete = false

exclude_patterns = [
    "*.bak",
    "~$*",
    "*.tmp",
    ".dropbox",
    "Thumbs.db",
    "desktop.ini",
]

[schedule]
enabled = false
frequency = "daily"   # "daily" | "weekly" | "monthly"
hour = 22
minute = 0
weekday = "mon"        # dung khi frequency = "weekly"
day_of_month = 1       # dung khi frequency = "monthly" (1-28)
```

Giải thích từng mục:

- **`source_root`** — đường dẫn gốc trên NAS. Kiểm tra lại đúng đường dẫn thật của bạn
  (ví dụ có thể là `\\Vci-bim-nas\projects` hoặc khác, tuỳ cấu trúc NAS).
- **`projects`** — danh sách **tên thư mục con** cần backup (theo yêu cầu: chỉ backup
  đúng danh sách này, không quét toàn bộ NAS). Sửa danh sách mẫu thành tên project thật
  của bạn, hoặc dùng ô "+ Thêm" trong giao diện rồi bấm "Lưu cấu hình".
- **`default_destination`** — thư mục đích gợi ý sẵn khi mở tool. Vẫn có thể đổi ổ khác
  bằng nút "Chọn ổ đích..." mỗi lần chạy mà không cần sửa file này.
- **`thread_count`** — số luồng copy song song. Để `0` = tool **tự động chọn** theo số
  nhân CPU của máy đang chạy (giới hạn 4–8, vì backup qua mạng là tác vụ phụ thuộc
  băng thông/NAS chứ không phải CPU — quá nhiều luồng dễ làm nghẽn thay vì nhanh hơn).
  Có thể chỉnh tay bằng thanh trượt trong giao diện (1–32).
- **`enable_mirror_delete`** — bạn đã chọn chế độ **Đồng bộ 1 chiều (Mirror)**, nghĩa
  là những file/thư mục đã bị xoá ở NAS sẽ được xoá luôn ở ổ Elements để đích luôn
  giống hệt nguồn. Vì đây là thao tác **xoá vĩnh viễn**, mặc định tool để `false` (an
  toàn) — bạn tự bật `true` trong giao diện (ô "Xoá file thừa ở đích") khi đã sẵn sàng
  dùng đúng chế độ mirror thật.
- **`exclude_patterns`** — các file/thư mục tạm không cần backup (file khoá tạm của
  Revit `~$*`, file rác Windows...). Hỗ trợ ký tự đại diện `*`.
- **`[schedule]`** — cấu hình backup tự động theo lịch (hằng ngày/tuần/tháng). Nên chỉnh
  qua giao diện (mục "LỊCH TỰ ĐỘNG" + nút "Lưu lịch tự động") thay vì sửa tay, vì tool
  cần đồng thời đăng ký tác vụ trong Windows Task Scheduler mỗi khi lịch thay đổi — xem
  mục 7 bên dưới.
- **`ui_dark_mode`** — giao diện Tối (`true`, mặc định) hay Sáng (`false`). Đổi nhanh bằng
  nút **☀/🌙** góc trên bên phải — tool tự lưu lại lựa chọn ngay khi bấm, không cần bấm
  "Lưu cấu hình".

## 4. Cách dùng

1. Mở `vci_backup.exe`.
2. Bấm **"Chọn ổ đích..."** và chọn thư mục gốc trên ổ Elements (ví dụ `E:\BIM_Backup`).
3. Tick chọn những project cần backup trong danh sách bên trái (mặc định tick hết).
4. (Tuỳ chọn) Bật **"Xoá file thừa ở đích (Mirror thật)"** nếu muốn ổ Elements luôn
   giống hệt NAS 100%. Nếu chỉ muốn thêm/cập nhật, an toàn hơn, để tắt.
5. Bấm **"▶ BẮT ĐẦU BACKUP"**.
6. Theo dõi thanh tiến độ, các thẻ thống kê (số file, dung lượng, tốc độ MB/s, thời
   gian còn lại ước tính), **biểu đồ tròn** thể hiện tỷ lệ Đã chép / Đã là mới nhất /
   Đã xoá / Lỗi, và nhật ký ở khung dưới.
7. Có thể bấm **"⏹ HUỶ BACKUP"** bất kỳ lúc nào — tool sẽ dừng nhận việc mới, để các
   file đang copy dở hoàn tất an toàn rồi dừng hẳn (không để lại file dở dang).
8. Bấm nút **☀ Sáng / 🌙 Tối** ở góc trên bên phải để đổi giao diện sáng/tối theo ý thích.

### Cách tool quyết định file nào cần copy

Tool so sánh **kích thước + thời gian sửa đổi (mtime)** giữa nguồn và đích (dung sai
2 giây để tương thích định dạng exFAT trên ổ cứng rời). File giống hệt sẽ được **bỏ
qua** (không copy lại), giúp các lần backup sau nhanh hơn rất nhiều so với lần đầu.
Tool không hash nội dung file vì dữ liệu BIM có thể rất nặng — hash sẽ chậm hơn nhiều
so với chỉ đọc metadata.

## 5. Hiệu năng & tối ưu hoá

| Hạng mục | Cách tối ưu |
|---|---|
| RAM | Buffer copy cố định 4MB/luồng, tái sử dụng; không tải nguyên file vào bộ nhớ |
| Tốc độ | Đa luồng copy song song (mặc định 4–8 luồng tự động theo CPU) |
| Tốc độ (lần sau) | Bỏ qua file không đổi (so sánh size + mtime), không copy lại |
| Đường dẫn dài | Tự động dùng đường dẫn dạng `\\?\` khi có thể, không giới hạn 260 ký tự |
| Giao diện | Chạy trên luồng riêng, không bao giờ bị đứng/treo khi đang copy hàng nghìn file |

## 6. Cấu trúc mã nguồn

```
src/
  main.rs      Điểm khởi động; xử lý cả chế độ chạy ngầm `--auto-backup`
  app.rs       Giao diện (egui) + điều phối trạng thái
  config.rs    Đọc/ghi config.toml (bao gồm cấu hình lịch tự động)
  model.rs     Các kiểu dữ liệu dùng chung (thống kê, thông điệp tiến trình)
  scanner.rs   Quét thư mục, so sánh, tính việc cần copy/xoá
  copier.rs    Engine backup: điều phối đa luồng + copy từng file
  patterns.rs  So khớp mẫu loại trừ (wildcard *)
  schedule.rs  Đăng ký/gỡ tác vụ trong Windows Task Scheduler
  theme.rs     Giao diện màu sắc, bo góc
```

Đã có sẵn bộ test tự động (`cargo test --release`) kiểm tra logic quét/so sánh/copy/xoá
để tránh xoá nhầm dữ liệu.

## 7. Backup tự động theo lịch (hằng ngày / hằng tuần / hằng tháng)

Khung **"LỊCH TỰ ĐỘNG"** ở cuối thanh bên trái cho phép bật backup chạy ngầm theo lịch,
không cần mở tool thủ công mỗi lần:

1. Bấm chọn ổ đích và tick các project cần backup như bình thường (những lựa chọn này
   sẽ được dùng cho cả các lần backup tự động).
2. Tick **"Bật backup tự động theo lịch"**.
3. Chọn tần suất: **Hằng ngày**, **Hằng tuần** (chọn thêm thứ) hoặc **Hằng tháng**
   (chọn thêm ngày trong tháng, giới hạn 1–28 để luôn chạy đủ mọi tháng kể cả tháng 2).
4. Chọn giờ chạy (giờ:phút).
5. Bấm **"⏰ Lưu lịch tự động"**.

Tool sẽ tạo 1 tác vụ trong **Windows Task Scheduler** (tên `VCI_BIM_Backup_Auto`) gọi
lại chính `vci_backup.exe` kèm cờ `--auto-backup` đúng giờ đã hẹn. Khi chạy ở chế độ
này, tool **không mở giao diện** — chỉ đọc `config.toml`, backup toàn bộ project đã cấu
hình, rồi ghi kết quả vào:

- `last_auto_backup.txt` (cạnh file `.exe`) — tóm tắt lần chạy gần nhất, tool sẽ tự đọc
  và hiện lại trong khung "Nhật ký" mỗi khi bạn mở giao diện lên.
- Thư mục `logs\` — lưu lịch sử từng lần chạy tự động, mỗi lần 1 file `.log`.

**Lưu ý quan trọng:**

- Máy tính phải **đang bật** (không tắt hẳn — ngủ đông/sleep thường vẫn được, Windows
  tự đánh thức máy đúng giờ hẹn nếu task được tạo bình thường) và ổ **Elements phải
  đang cắm vào cổng USB** đúng lúc lịch chạy thì lần backup đó mới thực sự diễn ra. Nếu
  ổ chưa cắm, tool tự bỏ qua lần đó một cách an toàn (không báo lỗi ầm ĩ) và sẽ backup
  bù vào lần hẹn kế tiếp.
- Việc tạo/xoá tác vụ trong Task Scheduler chỉ hoạt động trên **Windows**.
- Muốn tắt lịch: bỏ tick "Bật backup tự động theo lịch" rồi bấm lại **"⏰ Lưu lịch tự
  động"** — tool sẽ tự xoá tác vụ trong Task Scheduler.
- Có thể tự kiểm tra tác vụ đã tạo bằng cách mở **Task Scheduler** (gõ `taskschd.msc`)
  → tìm tác vụ tên `VCI_BIM_Backup_Auto`.

## 8. Lưu ý an toàn khi dùng chế độ Mirror

Chế độ mirror **xoá vĩnh viễn** ở ổ Elements những gì không còn ở NAS. Gợi ý:

- Lần đầu dùng, nên chạy **thử với `enable_mirror_delete = false`** trước để xem tool
  hoạt động đúng ý, sau đó mới bật xoá.
- Theo dõi khung "Nhật ký" — mọi thao tác xoá đều được ghi lại kèm giờ:phút:giây.
- Nếu lỡ xoá nhầm ở NAS trước khi backup, dữ liệu ở Elements cũng sẽ mất theo ở lần
  chạy tiếp theo — vì vậy ổ Elements nên được xem là **bản sao đồng bộ**, không phải
  bản lưu trữ nhiều phiên bản.
