# Issue Tracker BIM

Web app React + TypeScript + Tailwind CSS de theo doi y kien/issue cho du an xay dung va BIM.

## Chuc nang

- Bang issue responsive theo phong cach quan ly du an.
- Tao issue, tra loi issue, loc, tim kiem va phan trang 10/20 dong.
- Upload PDF, DOCX, XLSX, PNG, JPG, ZIP, DWG... hoac gan link Drive/SharePoint.
- Badge file theo dinh dang, mo/tai file truc tiep.
- Supabase Auth, PostgreSQL, Storage va RLS cho role `admin`, `editor`, `viewer`.
- Mock Data tu dong khi chua cau hinh Supabase.

## Chay local

```bash
npm install
npm run dev
```

## Ket noi Supabase va deploy mien phi

1. Tao project tai Supabase, mo SQL Editor va chay file `supabase/schema.sql`.
2. Vao `Settings > API` cua Supabase, lay `Project URL` va `anon public key`.
3. Vao `Settings > Secrets and variables > Actions` cua GitHub repository va tao hai secret:
   - `VITE_SUPABASE_URL`
   - `VITE_SUPABASE_ANON_KEY`
4. Vao `Settings > Pages`, chon source `GitHub Actions`.
5. Push code hoac bam `Run workflow` trong workflow `Deploy Issue Tracker`.
6. Dang ky tai khoan dau tien, lay UUID trong Supabase `Authentication > Users`, sau do chay lenh update admin o cuoi file SQL.

Khi chua co hai bien moi truong Supabase, app tu dong hien Mock Data de xem giao dien. Khi da cau hinh, app bat buoc dang nhap va RLS cua Supabase se bao ve quyen:

- `admin`: xem, them, sua, xoa issue va quan ly file.
- `editor`: xem, them, sua va tra loi issue.
- `viewer`: chi xem va tai file.

File upload duoc luu trong Storage bucket `issue-files`, khong luu vao may nguoi dung.
