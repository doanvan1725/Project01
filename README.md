# Revit Backup Inspector

App Python giao dien desktop de:

- quet cac file backup Revit trong mot thu muc
- thong ke so luong va dung luong
- loc tim nhanh theo ten, thu muc, loai backup
- xoa file backup da chon hoac xoa toan bo ket qua

## Chuc nang

- `Scan backup`: quet file theo duong dan ban chon
- `Quet ca thu muc con`: bat/tat quet recursive
- `Xoa muc dang chon`: xoa cac file dang duoc chon tren bang
- `Xoa toan bo ket qua`: xoa tat ca backup tim thay trong ket qua hien tai
- giao dien dark, card thong ke, bang ket qua, nhat ky xu ly

## Cach chay local

```bash
pip install -r requirements.txt
python app.py
```

## Build EXE tren GitHub Actions

Workflow trong `.github/workflows/build-windows.yml` se:

1. checkout source
2. cai Python
3. cai dependencies
4. dong goi bang PyInstaller
5. upload file `RevitBackupInspector.exe` nhu artifact

## Quy tac nhan dien file backup

App se gom:

- file co duoi `.rvt.bak`, `.rfa.bak`, `.rte.bak`, `.rft.bak`
- file co duoi so backup kieu `model.0001.rvt`, `family.0003.rfa`

## Luu y

- Xoa file la hanh dong khong the hoan tac.
- Nen scan truoc va kiem tra danh sach truoc khi xoa.
