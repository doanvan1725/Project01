from __future__ import annotations

import os
import re
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable

from PySide6.QtCore import QDateTime, QObject, QThread, Qt, Signal, Slot
from PySide6.QtGui import QColor, QFont, QIcon, QPalette
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QCheckBox,
    QComboBox,
    QFileDialog,
    QFrame,
    QGridLayout,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSizePolicy,
    QTableWidget,
    QTableWidgetItem,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)


BACKUP_EXTENSIONS = {".rvt", ".rfa", ".rte", ".rft"}
NUMBERED_BACKUP_RE = re.compile(r"\.\d{3,4}\.(?:rvt|rfa|rte|rft)$", re.IGNORECASE)


@dataclass(frozen=True)
class BackupItem:
    path: Path
    name: str
    folder: str
    size: int
    modified: datetime
    backup_type: str

    @property
    def modified_display(self) -> str:
        return self.modified.strftime("%Y-%m-%d %H:%M:%S")


def human_size(value: int) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    size = float(value)
    for unit in units:
        if size < 1024.0 or unit == units[-1]:
            return f"{int(size)} {unit}" if unit == "B" else f"{size:.1f} {unit}"
        size /= 1024.0
    return f"{value} B"


def is_revit_backup(path: Path) -> tuple[bool, str]:
    lower_name = path.name.lower()
    if lower_name.endswith(".bak") and any(lower_name.endswith(f"{ext}.bak") for ext in BACKUP_EXTENSIONS):
        return True, "Classic backup"
    if NUMBERED_BACKUP_RE.search(lower_name):
        return True, "Numbered backup"
    return False, ""


def scan_backups(
    root: Path,
    include_subfolders: bool,
    progress_cb=None,
    status_cb=None,
) -> list[BackupItem]:
    items: list[BackupItem] = []
    if include_subfolders:
        def walk_candidates() -> Iterable[Path]:
            for dirpath, _, filenames in os.walk(root, onerror=lambda _: None):
                base = Path(dirpath)
                for filename in filenames:
                    yield base / filename

        candidates = walk_candidates()
    else:
        try:
            candidates = (p for p in root.iterdir() if p.is_file())
        except OSError:
            candidates = []

    processed = 0
    for path in candidates:
        processed += 1
        if progress_cb and processed % 250 == 0:
            progress_cb(processed)
        if status_cb and processed % 500 == 0:
            status_cb(f"Scanning {processed} files...")

        try:
            matched, backup_type = is_revit_backup(path)
            if not matched:
                continue
            stat = path.stat()
            items.append(
                BackupItem(
                    path=path,
                    name=path.name,
                    folder=str(path.parent),
                    size=stat.st_size,
                    modified=datetime.fromtimestamp(stat.st_mtime),
                    backup_type=backup_type,
                )
            )
        except (OSError, PermissionError):
            continue

    items.sort(key=lambda item: (item.modified, item.name.lower()), reverse=True)
    return items


class ScanWorker(QObject):
    finished = Signal(list, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(self, root: str, include_subfolders: bool) -> None:
        super().__init__()
        self.root = Path(root)
        self.include_subfolders = include_subfolders

    @Slot()
    def run(self) -> None:
        try:
            self.status.emit("Preparing scan...")
            items = scan_backups(
                self.root,
                self.include_subfolders,
                progress_cb=lambda value: self.progress.emit(min(value, 100)),
                status_cb=self.status.emit,
            )
            self.finished.emit(items, 0)
        except Exception as exc:  # pragma: no cover - surfaced in UI
            self.status.emit(f"Scan failed: {exc}")
            self.finished.emit([], 1)


class DeleteWorker(QObject):
    finished = Signal(int, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(self, paths: list[Path]) -> None:
        super().__init__()
        self.paths = paths

    @Slot()
    def run(self) -> None:
        deleted = 0
        errors = 0
        total = max(1, len(self.paths))
        for index, path in enumerate(self.paths, start=1):
            try:
                path.unlink()
                deleted += 1
            except Exception:
                errors += 1
            self.progress.emit(int(index * 100 / total))
            self.status.emit(f"Deleting {index}/{total}...")
        self.finished.emit(deleted, errors)


class MetricCard(QFrame):
    def __init__(self, title: str, value: str, accent: str) -> None:
        super().__init__()
        self.setObjectName("metricCard")
        self.setMinimumHeight(104)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 16, 18, 16)
        layout.setSpacing(6)

        self.title_label = QLabel(title)
        self.title_label.setObjectName("metricTitle")
        self.value_label = QLabel(value)
        self.value_label.setObjectName("metricValue")
        self.accent_label = QLabel()
        self.accent_label.setFixedHeight(4)
        self.accent_label.setStyleSheet(f"background: {accent}; border-radius: 2px;")

        layout.addWidget(self.title_label)
        layout.addWidget(self.value_label)
        layout.addStretch(1)
        layout.addWidget(self.accent_label)

    def set_value(self, value: str) -> None:
        self.value_label.setText(value)


class MainWindow(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Revit Backup Inspector")
        self.resize(1260, 840)

        self.scan_thread: QThread | None = None
        self.scan_worker: ScanWorker | None = None
        self.delete_thread: QThread | None = None
        self.delete_worker: DeleteWorker | None = None
        self.backup_items: list[BackupItem] = []

        self._build_ui()
        self._apply_theme()

    def _build_ui(self) -> None:
        root = QWidget()
        self.setCentralWidget(root)
        root_layout = QVBoxLayout(root)
        root_layout.setContentsMargins(18, 18, 18, 18)
        root_layout.setSpacing(16)

        self.header = QFrame()
        self.header.setObjectName("heroPanel")
        header_layout = QVBoxLayout(self.header)
        header_layout.setContentsMargins(24, 24, 24, 24)
        header_layout.setSpacing(10)

        title = QLabel("Revit Backup Inspector")
        title.setObjectName("heroTitle")
        subtitle = QLabel(
            "Quet, thong ke va xoa file backup Revit trong mot giao dien de nhin, an toan va nhanh."
        )
        subtitle.setObjectName("heroSubtitle")

        self.path_label = QLabel("Chua chon thu muc")
        self.path_label.setObjectName("pathLabel")
        self.path_label.setWordWrap(True)

        header_layout.addWidget(title)
        header_layout.addWidget(subtitle)
        header_layout.addWidget(self.path_label)
        root_layout.addWidget(self.header)

        self.metric_cards = QFrame()
        cards_layout = QGridLayout(self.metric_cards)
        cards_layout.setContentsMargins(0, 0, 0, 0)
        cards_layout.setHorizontalSpacing(12)
        cards_layout.setVerticalSpacing(12)
        self.card_total = MetricCard("Tong backup", "0", "#4da3ff")
        self.card_size = MetricCard("Tong dung luong", "0 B", "#17c964")
        self.card_selected = MetricCard("Dang chon", "0", "#ffb020")
        self.card_deleted = MetricCard("Da xoa", "0", "#ff5c7a")
        cards_layout.addWidget(self.card_total, 0, 0)
        cards_layout.addWidget(self.card_size, 0, 1)
        cards_layout.addWidget(self.card_selected, 0, 2)
        cards_layout.addWidget(self.card_deleted, 0, 3)
        root_layout.addWidget(self.metric_cards)

        controls = QFrame()
        controls.setObjectName("panel")
        controls_layout = QGridLayout(controls)
        controls_layout.setContentsMargins(18, 18, 18, 18)
        controls_layout.setHorizontalSpacing(12)
        controls_layout.setVerticalSpacing(12)

        self.folder_edit = QLineEdit()
        self.folder_edit.setPlaceholderText("Chon thu muc chua backup Revit...")
        self.folder_edit.textChanged.connect(self._set_path_preview)

        browse_btn = QPushButton("Browse")
        browse_btn.clicked.connect(self.choose_folder)

        self.scan_subfolders = QCheckBox("Quet ca thu muc con")
        self.scan_subfolders.setChecked(True)

        self.search_edit = QLineEdit()
        self.search_edit.setPlaceholderText("Loc theo ten file, thu muc hoac loai backup...")
        self.search_edit.textChanged.connect(self._refresh_table_view)

        self.sort_combo = QComboBox()
        self.sort_combo.addItems(["Moi nhat truoc", "Lon nhat truoc", "Ten A-Z"])
        self.sort_combo.currentIndexChanged.connect(self._refresh_table_view)

        self.scan_button = QPushButton("Scan backup")
        self.scan_button.clicked.connect(self.start_scan)

        self.delete_selected_button = QPushButton("Xoa muc dang chon")
        self.delete_selected_button.clicked.connect(self.delete_selected)

        self.delete_all_button = QPushButton("Xoa toan bo ket qua")
        self.delete_all_button.clicked.connect(self.delete_all)

        controls_layout.addWidget(QLabel("Thu muc nguon"), 0, 0, 1, 2)
        controls_layout.addWidget(self.folder_edit, 1, 0, 1, 3)
        controls_layout.addWidget(browse_btn, 1, 3)
        controls_layout.addWidget(self.scan_subfolders, 2, 0)
        controls_layout.addWidget(QLabel("Tim nhanh"), 2, 1)
        controls_layout.addWidget(self.search_edit, 2, 2, 1, 2)
        controls_layout.addWidget(QLabel("Sap xep"), 3, 0)
        controls_layout.addWidget(self.sort_combo, 3, 1)
        controls_layout.addWidget(self.scan_button, 3, 2)
        controls_layout.addWidget(self.delete_selected_button, 3, 3)
        controls_layout.addWidget(self.delete_all_button, 4, 3)
        root_layout.addWidget(controls)

        table_wrap = QFrame()
        table_wrap.setObjectName("panel")
        table_layout = QVBoxLayout(table_wrap)
        table_layout.setContentsMargins(18, 18, 18, 18)
        table_layout.setSpacing(12)

        self.table = QTableWidget(0, 6)
        self.table.setHorizontalHeaderLabels(
            ["File", "Thu muc", "Loai", "Kich thuoc", "Cap nhat", "Duong dan"]
        )
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setAlternatingRowColors(True)
        self.table.setSortingEnabled(False)
        self.table.verticalHeader().setVisible(False)
        self.table.horizontalHeader().setStretchLastSection(True)
        self.table.horizontalHeader().setDefaultAlignment(Qt.AlignmentFlag.AlignLeft)
        self.table.setWordWrap(False)

        table_layout.addWidget(self.table)
        root_layout.addWidget(table_wrap, stretch=1)

        bottom = QFrame()
        bottom_layout = QVBoxLayout(bottom)
        bottom_layout.setContentsMargins(0, 0, 0, 0)
        bottom_layout.setSpacing(10)

        self.progress = QProgressBar()
        self.progress.setRange(0, 100)
        self.progress.setValue(0)

        self.log = QTextEdit()
        self.log.setReadOnly(True)
        self.log.setPlaceholderText("Nhat ky xu ly se hien o day...")
        self.log.setMaximumHeight(140)

        bottom_layout.addWidget(self.progress)
        bottom_layout.addWidget(self.log)
        root_layout.addWidget(bottom)

        self.statusBar().showMessage("San sang")
        self._set_default_folder()

    def _apply_theme(self) -> None:
        self.setFont(QFont("Segoe UI", 10))
        palette = QPalette()
        palette.setColor(QPalette.ColorRole.Window, QColor("#0f172a"))
        palette.setColor(QPalette.ColorRole.Base, QColor("#111827"))
        palette.setColor(QPalette.ColorRole.AlternateBase, QColor("#0b1220"))
        palette.setColor(QPalette.ColorRole.Text, QColor("#e5eefb"))
        palette.setColor(QPalette.ColorRole.WindowText, QColor("#e5eefb"))
        palette.setColor(QPalette.ColorRole.Button, QColor("#1d4ed8"))
        palette.setColor(QPalette.ColorRole.ButtonText, QColor("#ffffff"))
        palette.setColor(QPalette.ColorRole.Highlight, QColor("#38bdf8"))
        palette.setColor(QPalette.ColorRole.HighlightedText, QColor("#081018"))
        self.setPalette(palette)

        self.setStyleSheet(
            """
            QWidget {
                color: #e5eefb;
                font-family: "Segoe UI";
                font-size: 10pt;
            }
            QMainWindow {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #08111f, stop:1 #101b33);
            }
            #heroPanel, #panel, #metricCard {
                background: rgba(15, 23, 42, 0.88);
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 18px;
            }
            #heroTitle {
                font-size: 24pt;
                font-weight: 700;
                color: #f8fbff;
            }
            #heroSubtitle {
                color: #b5c3da;
                font-size: 10.5pt;
            }
            #pathLabel {
                color: #8ec8ff;
                padding-top: 4px;
            }
            #metricTitle {
                color: #9fb2ce;
                font-size: 9.5pt;
                letter-spacing: 0.4px;
            }
            #metricValue {
                font-size: 21pt;
                font-weight: 700;
                color: #ffffff;
            }
            QPushButton {
                background: #1f3fbf;
                border: 1px solid rgba(255, 255, 255, 0.12);
                padding: 10px 14px;
                border-radius: 12px;
                font-weight: 600;
            }
            QPushButton:hover {
                background: #2d53df;
            }
            QPushButton:pressed {
                background: #17339d;
            }
            QPushButton:disabled {
                color: #93a4bd;
                background: #23314c;
            }
            QLineEdit, QComboBox, QTextEdit {
                background: #0b1324;
                border: 1px solid rgba(148, 163, 184, 0.22);
                border-radius: 10px;
                padding: 9px 11px;
                selection-background-color: #38bdf8;
            }
            QComboBox::drop-down {
                border: 0px;
                width: 28px;
            }
            QTableWidget {
                background: #0b1220;
                border: 1px solid rgba(148, 163, 184, 0.18);
                border-radius: 14px;
                gridline-color: rgba(148, 163, 184, 0.12);
                selection-background-color: rgba(56, 189, 248, 0.18);
                selection-color: #ffffff;
            }
            QHeaderView::section {
                background: #12203b;
                color: #d6e3fb;
                padding: 10px 8px;
                border: 0px;
                border-bottom: 1px solid rgba(148, 163, 184, 0.18);
                font-weight: 600;
            }
            QCheckBox {
                spacing: 8px;
            }
            QCheckBox::indicator {
                width: 18px;
                height: 18px;
                border-radius: 5px;
                border: 1px solid rgba(148, 163, 184, 0.45);
                background: #0b1324;
            }
            QCheckBox::indicator:checked {
                background: #38bdf8;
                border-color: #38bdf8;
            }
            QProgressBar {
                border: 1px solid rgba(148, 163, 184, 0.22);
                border-radius: 8px;
                text-align: center;
                background: #0b1324;
                height: 18px;
            }
            QProgressBar::chunk {
                border-radius: 8px;
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #3b82f6, stop:1 #22c55e);
            }
            """
        )

    def _set_default_folder(self) -> None:
        default = Path.home() / "Documents"
        self.folder_edit.setText(str(default if default.exists() else Path.home()))

    def _set_path_preview(self, text: str) -> None:
        self.path_label.setText(text.strip() or "Chua chon thu muc")

    def choose_folder(self) -> None:
        folder = QFileDialog.getExistingDirectory(self, "Chon thu muc Revit backup")
        if folder:
            self.folder_edit.setText(folder)

    def _set_ui_busy(self, busy: bool) -> None:
        self.scan_button.setDisabled(busy)
        self.delete_selected_button.setDisabled(busy)
        self.delete_all_button.setDisabled(busy)
        self.folder_edit.setDisabled(busy)
        self.scan_subfolders.setDisabled(busy)
        self.search_edit.setDisabled(busy)
        self.sort_combo.setDisabled(busy)

    def _append_log(self, message: str) -> None:
        now = QDateTime.currentDateTime().toString("HH:mm:ss")
        self.log.append(f"[{now}] {message}")
        self.statusBar().showMessage(message)

    def start_scan(self) -> None:
        root_text = self.folder_edit.text().strip()
        root = Path(root_text)
        if not root_text:
            QMessageBox.warning(self, "Thieu duong dan", "Hay chon thu muc can quet.")
            return
        if not root.exists():
            QMessageBox.warning(self, "Khong ton tai", "Thu muc da chon khong ton tai.")
            return

        self._set_ui_busy(True)
        self.progress.setValue(0)
        self._append_log(f"Bat dau quet: {root}")

        self.scan_thread = QThread(self)
        self.scan_worker = ScanWorker(str(root), self.scan_subfolders.isChecked())
        self.scan_worker.moveToThread(self.scan_thread)
        self.scan_thread.started.connect(self.scan_worker.run)
        self.scan_worker.status.connect(self._append_log)
        self.scan_worker.progress.connect(self.progress.setValue)
        self.scan_worker.finished.connect(self._scan_finished)
        self.scan_worker.finished.connect(self.scan_thread.quit)
        self.scan_worker.finished.connect(self.scan_worker.deleteLater)
        self.scan_thread.finished.connect(self.scan_thread.deleteLater)
        self.scan_thread.start()

    @Slot(list, int)
    def _scan_finished(self, items: list[BackupItem], error_code: int) -> None:
        self._set_ui_busy(False)
        self.progress.setValue(100 if error_code == 0 else 0)
        self.backup_items = items if error_code == 0 else []
        self._refresh_table_view()
        if error_code == 0:
            self._append_log(f"Hoan tat quet. Tim thay {len(self.backup_items)} file backup.")
        else:
            self._append_log("Quet that bai.")

    def _filtered_and_sorted_items(self) -> list[BackupItem]:
        query = self.search_edit.text().strip().lower()
        filtered = [
            item
            for item in self.backup_items
            if not query
            or query in item.name.lower()
            or query in item.folder.lower()
            or query in item.backup_type.lower()
            or query in str(item.path).lower()
        ]
        mode = self.sort_combo.currentIndex()
        if mode == 1:
            filtered.sort(key=lambda item: item.size, reverse=True)
        elif mode == 2:
            filtered.sort(key=lambda item: item.name.lower())
        else:
            filtered.sort(key=lambda item: item.modified, reverse=True)
        return filtered

    def _selected_rows(self) -> list[int]:
        selection = self.table.selectionModel()
        if selection is None:
            return []
        return sorted({index.row() for index in selection.selectedRows()})

    def _selected_items(self) -> list[BackupItem]:
        visible = self._filtered_and_sorted_items()
        return [visible[row] for row in self._selected_rows() if 0 <= row < len(visible)]

    def _refresh_table_view(self) -> None:
        items = self._filtered_and_sorted_items()
        self.table.setRowCount(len(items))
        for row, item in enumerate(items):
            values = [
                item.name,
                item.folder,
                item.backup_type,
                human_size(item.size),
                item.modified_display,
                str(item.path),
            ]
            for col, value in enumerate(values):
                table_item = QTableWidgetItem(value)
                if col == 3:
                    table_item.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                if col in {3, 4}:
                    table_item.setForeground(QColor("#cfe3ff"))
                self.table.setItem(row, col, table_item)

        self.table.resizeColumnsToContents()
        self.table.setColumnWidth(1, max(220, self.table.columnWidth(1)))
        self.table.setColumnWidth(5, max(260, self.table.columnWidth(5)))
        total_size = sum(item.size for item in self.backup_items)
        self.card_total.set_value(f"{len(self.backup_items):,}")
        self.card_size.set_value(human_size(total_size))
        self.card_selected.set_value(f"{len(self._selected_rows()):,}")

    def delete_selected(self) -> None:
        items = self._selected_items()
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon it nhat mot file backup can xoa.")
            return
        self._confirm_and_delete(items)

    def delete_all(self) -> None:
        items = self._filtered_and_sorted_items()
        if not items:
            QMessageBox.information(self, "Khong co gi", "Khong co file backup nao trong ket qua hien tai.")
            return
        self._confirm_and_delete(items)

    def _confirm_and_delete(self, items: list[BackupItem]) -> None:
        total_size = sum(item.size for item in items)
        reply = QMessageBox.warning(
            self,
            "Xac nhan xoa",
            (
                f"Ban sap xoa {len(items)} file backup, tong dung luong {human_size(total_size)}.\n\n"
                "Hanh dong nay khong the hoan tac. Ban co muon tiep tuc?"
            ),
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if reply != QMessageBox.StandardButton.Yes:
            return

        self._set_ui_busy(True)
        self.progress.setValue(0)
        self._append_log(f"Bat dau xoa {len(items)} file...")

        self.delete_thread = QThread(self)
        self.delete_worker = DeleteWorker([item.path for item in items])
        self.delete_worker.moveToThread(self.delete_thread)
        self.delete_thread.started.connect(self.delete_worker.run)
        self.delete_worker.status.connect(self._append_log)
        self.delete_worker.progress.connect(self.progress.setValue)
        self.delete_worker.finished.connect(self._delete_finished)
        self.delete_worker.finished.connect(self.delete_thread.quit)
        self.delete_worker.finished.connect(self.delete_worker.deleteLater)
        self.delete_thread.finished.connect(self.delete_thread.deleteLater)
        self.delete_thread.start()

    @Slot(int, int)
    def _delete_finished(self, deleted: int, errors: int) -> None:
        self._set_ui_busy(False)
        self.progress.setValue(100)
        self.card_deleted.set_value(f"{deleted:,}")
        self._append_log(f"Da xoa {deleted} file, loi {errors} file.")
        self.start_scan()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("Revit Backup Inspector")
    app.setOrganizationName("DoanVan1725")
    app.setWindowIcon(QIcon())
    window = MainWindow()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
