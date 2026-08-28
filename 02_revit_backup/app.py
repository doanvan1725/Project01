from __future__ import annotations

import os
import re
import shutil
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable

from PySide6.QtCore import QDateTime, QObject, QThread, Qt, QUrl, Signal, Slot
from PySide6.QtGui import QColor, QDesktopServices, QFont, QIcon, QLinearGradient, QPainter, QPalette, QPixmap, QBrush
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QCheckBox,
    QComboBox,
    QFileDialog,
    QFrame,
    QGridLayout,
    QHBoxLayout,
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


def runtime_dir() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent


def safe_resolve(path: Path) -> Path:
    return path.resolve(strict=False)


def is_under(path: Path, root: Path) -> bool:
    path_resolved = safe_resolve(path)
    root_resolved = safe_resolve(root)
    return path_resolved == root_resolved or path_resolved.is_relative_to(root_resolved)


def is_revit_backup(path: Path, include_autocad_bak: bool = True) -> tuple[bool, str]:
    lower_name = path.name.lower()
    if lower_name.endswith(".bak") and any(lower_name.endswith(f"{ext}.bak") for ext in BACKUP_EXTENSIONS):
        return True, "Classic backup"
    if include_autocad_bak and lower_name.endswith(".bak"):
        return True, "AutoCAD backup (.bak)"
    if NUMBERED_BACKUP_RE.search(lower_name):
        return True, "Numbered backup"
    return False, ""


def unique_destination_path(destination: Path) -> Path:
    if not destination.exists():
        return destination

    stem = destination.stem
    suffix = destination.suffix
    parent = destination.parent
    for index in range(1, 10_000):
        candidate = parent / f"{stem} ({index}){suffix}"
        if not candidate.exists():
            return candidate
    raise FileExistsError(f"Khong tao duoc ten moi cho {destination}")


def iter_candidates(root: Path, include_subfolders: bool, exclude_dirs: list[Path]) -> Iterable[Path]:
    exclude_dirs = [safe_resolve(p) for p in exclude_dirs if p]

    def excluded(path: Path) -> bool:
        return any(is_under(path, ex) for ex in exclude_dirs)

    if include_subfolders:
        for dirpath, dirs, filenames in os.walk(root, onerror=lambda _: None):
            base = Path(dirpath)
            if excluded(base):
                dirs[:] = []
                continue

            kept_dirs: list[str] = []
            for dirname in dirs:
                candidate_dir = base / dirname
                if not excluded(candidate_dir):
                    kept_dirs.append(dirname)
            dirs[:] = kept_dirs

            for filename in filenames:
                candidate = base / filename
                if not excluded(candidate):
                    yield candidate
    else:
        try:
            for path in root.iterdir():
                if path.is_file() and not excluded(path):
                    yield path
        except OSError:
            return


def scan_backups(
    root: Path,
    include_subfolders: bool,
    exclude_dirs: list[Path] | None = None,
    progress_cb=None,
    status_cb=None,
    include_autocad_bak: bool = True,
) -> list[BackupItem]:
    items: list[BackupItem] = []
    exclude_dirs = exclude_dirs or []

    processed = 0
    for path in iter_candidates(root, include_subfolders, exclude_dirs):
        processed += 1
        if progress_cb and processed % 250 == 0:
            progress_cb(processed)
        if status_cb and processed % 500 == 0:
            status_cb(f"Scanning {processed} files...")

        try:
            matched, backup_type = is_revit_backup(path, include_autocad_bak)
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


def create_app_icon() -> QIcon:
    pixmap = QPixmap(256, 256)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    gradient = QLinearGradient(0, 0, 256, 256)
    gradient.setColorAt(0.0, QColor("#1e40af"))
    gradient.setColorAt(0.55, QColor("#0f172a"))
    gradient.setColorAt(1.0, QColor("#0ea5e9"))
    painter.setBrush(gradient)
    painter.setPen(Qt.PenStyle.NoPen)
    painter.drawRoundedRect(12, 12, 232, 232, 48, 48)
    painter.setPen(QColor("#eaf4ff"))
    font = QFont("Segoe UI", 88, QFont.Weight.Bold)
    painter.setFont(font)
    painter.drawText(pixmap.rect(), Qt.AlignmentFlag.AlignCenter, "RB")
    painter.end()
    return QIcon(pixmap)


class FolderLineEdit(QLineEdit):
    def __init__(self, placeholder: str = "") -> None:
        super().__init__()
        self.setPlaceholderText(placeholder)
        self.setAcceptDrops(True)

    def dragEnterEvent(self, event) -> None:  # noqa: N802
        if event.mimeData().hasUrls():
            event.acceptProposedAction()
            return
        super().dragEnterEvent(event)

    def dropEvent(self, event) -> None:  # noqa: N802
        urls = event.mimeData().urls()
        if not urls:
            super().dropEvent(event)
            return

        local_path = Path(urls[0].toLocalFile())
        if local_path.is_file():
            local_path = local_path.parent
        self.setText(str(local_path))
        event.acceptProposedAction()


class ScanWorker(QObject):
    finished = Signal(object, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(
        self,
        root: str,
        include_subfolders: bool,
        include_autocad_bak: bool,
        exclude_dirs: list[str],
    ) -> None:
        super().__init__()
        self.root = Path(root)
        self.include_subfolders = include_subfolders
        self.include_autocad_bak = include_autocad_bak
        self.exclude_dirs = [Path(p) for p in exclude_dirs if p]

    @Slot()
    def run(self) -> None:
        try:
            self.status.emit("Preparing scan...")
            items = scan_backups(
                self.root,
                self.include_subfolders,
                self.exclude_dirs,
                progress_cb=lambda value: self.progress.emit(min(value, 100)),
                status_cb=self.status.emit,
                include_autocad_bak=self.include_autocad_bak,
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
            except Exception as exc:
                errors += 1
                self.status.emit(f"Delete failed: {path.name} ({exc})")
            self.progress.emit(int(index * 100 / total))
            self.status.emit(f"Deleting {index}/{total}...")
        self.finished.emit(deleted, errors)


class MoveWorker(QObject):
    finished = Signal(int, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(self, paths: list[Path], destination_dir: Path) -> None:
        super().__init__()
        self.paths = paths
        self.destination_dir = destination_dir

    @Slot()
    def run(self) -> None:
        moved = 0
        errors = 0
        total = max(1, len(self.paths))
        self.destination_dir.mkdir(parents=True, exist_ok=True)
        for index, src in enumerate(self.paths, start=1):
            try:
                dst = unique_destination_path(self.destination_dir / src.name)
                shutil.move(str(src), str(dst))
                moved += 1
            except Exception as exc:
                errors += 1
                self.status.emit(f"Move failed: {src.name} ({exc})")
            self.progress.emit(int(index * 100 / total))
            self.status.emit(f"Moving {index}/{total}...")
        self.finished.emit(moved, errors)


class MetricCard(QFrame):
    def __init__(self, title: str, value: str, accent: str) -> None:
        super().__init__()
        self.setObjectName("metricCard")
        self.setMinimumHeight(106)
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
        self.resize(1360, 880)

        self.scan_thread: QThread | None = None
        self.scan_worker: ScanWorker | None = None
        self.delete_thread: QThread | None = None
        self.delete_worker: DeleteWorker | None = None
        self.move_thread: QThread | None = None
        self.move_worker: MoveWorker | None = None
        self.backup_items: list[BackupItem] = []
        self.staging_marked: set[Path] = set()
        self.log_path = self._init_log_file()

        self._build_ui()
        self._apply_theme()
        self._log(f"App started - log file: {self.log_path}")

    def _init_log_file(self) -> Path:
        logs_dir = runtime_dir() / "logs"
        logs_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        return logs_dir / f"revit_backup_{stamp}.log"

    def _build_ui(self) -> None:
        root = QWidget()
        self.setCentralWidget(root)
        root_layout = QVBoxLayout(root)
        root_layout.setContentsMargins(18, 18, 18, 18)
        root_layout.setSpacing(16)

        self.header = QFrame()
        self.header.setObjectName("heroPanel")
        header_layout = QHBoxLayout(self.header)
        header_layout.setContentsMargins(24, 24, 24, 24)
        header_layout.setSpacing(18)

        badge = QLabel("RB")
        badge.setObjectName("appBadge")
        badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        badge.setFixedSize(72, 72)

        header_text = QVBoxLayout()
        header_text.setSpacing(8)
        title = QLabel("Revit Backup Inspector")
        title.setObjectName("heroTitle")
        subtitle = QLabel(
            "Quet, loc, chuyen va xoa file backup Revit trong mot giao dien sang, ro va an toan."
        )
        subtitle.setObjectName("heroSubtitle")
        self.path_label = QLabel("Chua chon thu muc nguon")
        self.path_label.setObjectName("pathLabel")
        self.path_label.setWordWrap(True)
        header_text.addWidget(title)
        header_text.addWidget(subtitle)
        header_text.addWidget(self.path_label)

        header_layout.addWidget(badge)
        header_layout.addLayout(header_text, stretch=1)
        root_layout.addWidget(self.header)

        self.metric_cards = QFrame()
        cards_layout = QGridLayout(self.metric_cards)
        cards_layout.setContentsMargins(0, 0, 0, 0)
        cards_layout.setHorizontalSpacing(12)
        cards_layout.setVerticalSpacing(12)
        self.card_total = MetricCard("Tong backup", "0", "#4da3ff")
        self.card_size = MetricCard("Tong dung luong", "0 B", "#17c964")
        self.card_selected = MetricCard("Dang chon", "0", "#ffb020")
        self.card_staged = MetricCard("Cho staging", "0", "#a855f7")
        cards_layout.addWidget(self.card_total, 0, 0)
        cards_layout.addWidget(self.card_size, 0, 1)
        cards_layout.addWidget(self.card_selected, 0, 2)
        cards_layout.addWidget(self.card_staged, 0, 3)
        root_layout.addWidget(self.metric_cards)

        controls = QFrame()
        controls.setObjectName("panel")
        controls_layout = QGridLayout(controls)
        controls_layout.setContentsMargins(18, 18, 18, 18)
        controls_layout.setHorizontalSpacing(12)
        controls_layout.setVerticalSpacing(12)

        self.folder_edit = FolderLineEdit("Keo-tha thu muc nguon vao day hoac bam Browse...")
        self.folder_edit.textChanged.connect(self._set_path_preview)

        browse_btn = QPushButton("Browse")
        browse_btn.clicked.connect(self.choose_folder)

        self.staging_edit = FolderLineEdit("Thu muc staging de chuyen backup vao day...")

        staging_browse_btn = QPushButton("Browse")
        staging_browse_btn.clicked.connect(self.choose_staging_folder)

        self.scan_subfolders = QCheckBox("Quet ca thu muc con")
        self.scan_subfolders.setChecked(True)

        self.scan_autocad_bak = QCheckBox("Nhan dien AutoCAD .bak")
        self.scan_autocad_bak.setChecked(True)

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

        self.select_all_button = QPushButton("Chon tat ca")
        self.select_all_button.clicked.connect(self.select_all_rows)

        self.clear_selection_button = QPushButton("Bo chon tat ca")
        self.clear_selection_button.clicked.connect(self.clear_all_rows)

        self.delete_all_button = QPushButton("Xoa toan bo ket qua")
        self.delete_all_button.clicked.connect(self.delete_all)

        self.move_selected_button = QPushButton("Chuyen muc chon vao staging")
        self.move_selected_button.clicked.connect(self.move_selected_to_staging)

        self.mark_staging_button = QPushButton("Danh dau staging")
        self.mark_staging_button.clicked.connect(self.mark_selected_for_staging)

        self.unmark_staging_button = QPushButton("Bo danh dau")
        self.unmark_staging_button.clicked.connect(self.unmark_selected_for_staging)

        self.move_marked_button = QPushButton("Chuyen muc da danh dau")
        self.move_marked_button.clicked.connect(self.move_marked_to_staging)

        self.move_all_button = QPushButton("Chuyen tat ca vao staging")
        self.move_all_button.clicked.connect(self.move_all_to_staging)

        self.open_source_button = QPushButton("Mo thu muc nguon")
        self.open_source_button.clicked.connect(self.open_source_folder)

        self.open_staging_button = QPushButton("Mo staging")
        self.open_staging_button.clicked.connect(self.open_staging_folder)

        self.open_selected_button = QPushButton("Mo thu muc file dang chon")
        self.open_selected_button.clicked.connect(self.open_selected_folder)

        controls_layout.addWidget(QLabel("Thu muc nguon"), 0, 0, 1, 2)
        controls_layout.addWidget(self.folder_edit, 1, 0, 1, 3)
        controls_layout.addWidget(browse_btn, 1, 3)
        controls_layout.addWidget(self.scan_subfolders, 2, 0)
        controls_layout.addWidget(self.scan_autocad_bak, 2, 1)
        controls_layout.addWidget(QLabel("Tim nhanh"), 2, 2)
        controls_layout.addWidget(self.search_edit, 2, 3)
        controls_layout.addWidget(QLabel("Sap xep"), 3, 0)
        controls_layout.addWidget(self.sort_combo, 3, 1)
        controls_layout.addWidget(self.scan_button, 3, 2)
        controls_layout.addWidget(self.delete_selected_button, 3, 3)
        controls_layout.addWidget(self.select_all_button, 4, 0)
        controls_layout.addWidget(self.clear_selection_button, 4, 1)
        controls_layout.addWidget(self.delete_all_button, 4, 2)
        controls_layout.addWidget(self.open_selected_button, 4, 3)
        controls_layout.addWidget(QLabel("Thu muc staging"), 5, 0, 1, 2)
        controls_layout.addWidget(self.staging_edit, 6, 0, 1, 3)
        controls_layout.addWidget(staging_browse_btn, 6, 3)
        controls_layout.addWidget(self.mark_staging_button, 7, 0)
        controls_layout.addWidget(self.unmark_staging_button, 7, 1)
        controls_layout.addWidget(self.move_marked_button, 7, 2)
        controls_layout.addWidget(self.move_selected_button, 7, 3)
        controls_layout.addWidget(self.move_all_button, 8, 0)
        controls_layout.addWidget(self.open_source_button, 8, 1)
        controls_layout.addWidget(self.open_staging_button, 8, 2)
        root_layout.addWidget(controls)

        table_wrap = QFrame()
        table_wrap.setObjectName("panel")
        table_layout = QVBoxLayout(table_wrap)
        table_layout.setContentsMargins(18, 18, 18, 18)
        table_layout.setSpacing(12)

        self.table = QTableWidget(0, 7)
        self.table.setHorizontalHeaderLabels(
            ["File", "Thu muc", "Loai", "Kich thuoc", "Cap nhat", "Trang thai", "Duong dan"]
        )
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setAlternatingRowColors(True)
        self.table.setSortingEnabled(False)
        self.table.verticalHeader().setVisible(False)
        self.table.horizontalHeader().setStretchLastSection(True)
        self.table.horizontalHeader().setDefaultAlignment(Qt.AlignmentFlag.AlignLeft)
        self.table.setWordWrap(False)
        self.table.itemDoubleClicked.connect(lambda _item: self.open_selected_folder())

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
        self.log.setMaximumHeight(160)

        bottom_layout.addWidget(self.progress)
        bottom_layout.addWidget(self.log)
        root_layout.addWidget(bottom)

        self.statusBar().showMessage("San sang")
        self._set_default_paths()

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
            #appBadge {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #3b82f6, stop:1 #8b5cf6);
                color: white;
                border-radius: 18px;
                font-size: 22pt;
                font-weight: 800;
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

    def _set_default_paths(self) -> None:
        default_source = Path.home() / "Documents"
        self.folder_edit.setText(str(default_source if default_source.exists() else Path.home()))
        self.staging_edit.setText(str(runtime_dir() / "revit_backup_stage"))

    def _set_path_preview(self, text: str) -> None:
        self.path_label.setText(text.strip() or "Chua chon thu muc nguon")

    def _log(self, message: str) -> None:
        now = QDateTime.currentDateTime().toString("HH:mm:ss")
        line = f"[{now}] {message}"
        self.log.append(line)
        self.statusBar().showMessage(message)
        try:
            with self.log_path.open("a", encoding="utf-8") as handle:
                handle.write(line + "\n")
        except OSError:
            pass

    def _set_ui_busy(self, busy: bool) -> None:
        self.scan_button.setDisabled(busy)
        self.delete_selected_button.setDisabled(busy)
        self.delete_all_button.setDisabled(busy)
        self.move_selected_button.setDisabled(busy)
        self.move_all_button.setDisabled(busy)
        self.folder_edit.setDisabled(busy)
        self.staging_edit.setDisabled(busy)
        self.scan_subfolders.setDisabled(busy)
        self.scan_autocad_bak.setDisabled(busy)
        self.search_edit.setDisabled(busy)
        self.sort_combo.setDisabled(busy)
        self.select_all_button.setDisabled(busy)
        self.clear_selection_button.setDisabled(busy)
        self.mark_staging_button.setDisabled(busy)
        self.unmark_staging_button.setDisabled(busy)
        self.move_marked_button.setDisabled(busy)

    def choose_folder(self) -> None:
        folder = QFileDialog.getExistingDirectory(self, "Chon thu muc nguon Revit backup")
        if folder:
            self.folder_edit.setText(folder)

    def choose_staging_folder(self) -> None:
        folder = QFileDialog.getExistingDirectory(self, "Chon thu muc staging")
        if folder:
            self.staging_edit.setText(folder)

    def source_root(self) -> Path | None:
        text = self.folder_edit.text().strip()
        if not text:
            return None
        return Path(text)

    def staging_root(self) -> Path | None:
        text = self.staging_edit.text().strip()
        if not text:
            return None
        return Path(text)

    def open_folder(self, path: Path | None) -> None:
        if path is None:
            QMessageBox.information(self, "Thieu duong dan", "Hay chon duong dan truoc.")
            return
        if not path.exists():
            QMessageBox.information(self, "Khong ton tai", f"Thu muc khong ton tai:\n{path}")
            return
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(path)))

    def open_source_folder(self) -> None:
        root = self.source_root()
        if root is not None:
            self.open_folder(root)

    def open_staging_folder(self) -> None:
        root = self.staging_root()
        if root is not None:
            self.open_folder(root)

    def _selected_rows(self) -> list[int]:
        selection = self.table.selectionModel()
        if selection is None:
            return []
        return sorted({index.row() for index in selection.selectedRows()})

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

    def _selected_items(self) -> list[BackupItem]:
        visible = self._filtered_and_sorted_items()
        return [visible[row] for row in self._selected_rows() if 0 <= row < len(visible)]

    def select_all_rows(self) -> None:
        self.table.selectAll()

    def clear_all_rows(self) -> None:
        self.table.clearSelection()

    def mark_selected_for_staging(self) -> None:
        items = self._selected_items()
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon it nhat mot file de danh dau staging.")
            return
        self.staging_marked.update(item.path for item in items)
        self._log(f"Da danh dau {len(items)} file se move sang staging.")
        self._refresh_table_view()

    def unmark_selected_for_staging(self) -> None:
        items = self._selected_items()
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon it nhat mot file de bo danh dau.")
            return
        for item in items:
            self.staging_marked.discard(item.path)
        self._log(f"Da bo danh dau staging cho {len(items)} file.")
        self._refresh_table_view()

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
                "Se move sang staging" if item.path in self.staging_marked else "",
                str(item.path),
            ]
            for col, value in enumerate(values):
                table_item = QTableWidgetItem(value)
                if col == 3:
                    table_item.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                if col in {3, 4}:
                    table_item.setForeground(QColor("#cfe3ff"))
                if item.path in self.staging_marked:
                    table_item.setBackground(QBrush(QColor("#4c1d95")))
                    table_item.setForeground(QColor("#f5e9ff"))
                self.table.setItem(row, col, table_item)

        self.table.resizeColumnsToContents()
        self.table.setColumnWidth(1, max(220, self.table.columnWidth(1)))
        self.table.setColumnWidth(5, max(150, self.table.columnWidth(5)))
        self.table.setColumnWidth(6, max(280, self.table.columnWidth(6)))
        total_size = sum(item.size for item in self.backup_items)
        self.card_total.set_value(f"{len(self.backup_items):,}")
        self.card_size.set_value(human_size(total_size))
        self.card_selected.set_value(f"{len(self._selected_rows()):,}")
        stage_dir = self.staging_root()
        self.card_staged.set_value(f"{len(self.staging_marked):,}" if stage_dir else "Unset")

    def start_scan(self) -> None:
        root = self.source_root()
        if root is None:
            QMessageBox.warning(self, "Thieu duong dan", "Hay chon thu muc can quet.")
            return
        if not root.exists():
            QMessageBox.warning(self, "Khong ton tai", "Thu muc nguon da chon khong ton tai.")
            return

        staging = self.staging_root()
        excludes: list[str] = []
        if staging and staging.exists() and is_under(staging, root):
            excludes.append(str(staging))

        self._set_ui_busy(True)
        self.progress.setValue(0)
        self._log(f"Bat dau quet: {root}")
        if excludes:
            self._log(f"Bo qua staging trong khi quet: {staging}")

        self.scan_thread = QThread(self)
        self.scan_worker = ScanWorker(
            str(root),
            self.scan_subfolders.isChecked(),
            self.scan_autocad_bak.isChecked(),
            excludes,
        )
        self.scan_worker.moveToThread(self.scan_thread)
        self.scan_thread.started.connect(self.scan_worker.run)
        self.scan_worker.status.connect(self._log)
        self.scan_worker.progress.connect(self.progress.setValue)
        self.scan_worker.finished.connect(self._scan_finished)
        self.scan_worker.finished.connect(self.scan_thread.quit)
        self.scan_worker.finished.connect(self.scan_worker.deleteLater)
        self.scan_thread.finished.connect(self.scan_thread.deleteLater)
        self.scan_thread.start()

    @Slot(object, int)
    def _scan_finished(self, items: object, error_code: int) -> None:
        self._set_ui_busy(False)
        self.progress.setValue(100 if error_code == 0 else 0)
        self.backup_items = items if error_code == 0 else []
        self._refresh_table_view()
        if error_code == 0:
            self._log(f"Hoan tat quet. Tim thay {len(self.backup_items)} file backup.")
        else:
            self._log("Quet that bai.")

    def open_selected_folder(self) -> None:
        items = self._selected_items()
        if not items:
            self.open_source_folder()
            return
        self.open_folder(items[0].path.parent)

    def _confirm_action(self, title: str, text: str) -> bool:
        reply = QMessageBox.warning(
            self,
            title,
            text,
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        return reply == QMessageBox.StandardButton.Yes

    def _confirm_target_folder(self, target: Path) -> bool:
        if target.exists():
            return True
        return self._confirm_action(
            "Tao thu muc",
            f"Thu muc nay chua ton tai:\n{target}\n\nBan co muon tao moi no khong?",
        )

    def _perform_delete(self, items: list[BackupItem]) -> None:
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon it nhat mot file backup can xoa.")
            return

        total_size = sum(item.size for item in items)
        if not self._confirm_action(
            "Xac nhan xoa",
            (
                f"Ban sap xoa {len(items)} file backup, tong dung luong {human_size(total_size)}.\n\n"
                "Hanh dong nay khong the hoan tac. Ban co muon tiep tuc?"
            ),
        ):
            return

        self._set_ui_busy(True)
        self.progress.setValue(0)
        self._log(f"Bat dau xoa {len(items)} file...")

        self.delete_thread = QThread(self)
        self.delete_worker = DeleteWorker([item.path for item in items])
        self.delete_worker.moveToThread(self.delete_thread)
        self.delete_thread.started.connect(self.delete_worker.run)
        self.delete_worker.status.connect(self._log)
        self.delete_worker.progress.connect(self.progress.setValue)
        self.delete_worker.finished.connect(self._delete_finished)
        self.delete_worker.finished.connect(self.delete_thread.quit)
        self.delete_worker.finished.connect(self.delete_worker.deleteLater)
        self.delete_thread.finished.connect(self.delete_thread.deleteLater)
        self.delete_thread.start()

    def delete_selected(self) -> None:
        self._perform_delete(self._selected_items())

    def delete_all(self) -> None:
        self._perform_delete(self._filtered_and_sorted_items())

    @Slot(int, int)
    def _delete_finished(self, deleted: int, errors: int) -> None:
        self._set_ui_busy(False)
        self.progress.setValue(100)
        self._log(f"Da xoa {deleted} file, loi {errors} file.")
        self.start_scan()

    def _perform_move_to_staging(self, items: list[BackupItem]) -> None:
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon it nhat mot file backup can chuyen.")
            return

        staging = self.staging_root()
        if staging is None:
            QMessageBox.warning(self, "Thieu duong dan", "Hay chon thu muc staging truoc.")
            return
        if not self._confirm_target_folder(staging):
            return

        total_size = sum(item.size for item in items)
        if not self._confirm_action(
            "Chuyen vao staging",
            (
                f"Ban sap chuyen {len(items)} file backup, tong dung luong {human_size(total_size)}\n"
                f"vao:\n{staging}\n\n"
                "Day la hanh dong cut/move, khong the hoan tac nhan cau truc."
            ),
        ):
            return

        self._set_ui_busy(True)
        self.progress.setValue(0)
        self._log(f"Bat dau chuyen {len(items)} file vao staging: {staging}")

        self.move_thread = QThread(self)
        self.move_worker = MoveWorker([item.path for item in items], staging)
        self.move_worker.moveToThread(self.move_thread)
        self.move_thread.started.connect(self.move_worker.run)
        self.move_worker.status.connect(self._log)
        self.move_worker.progress.connect(self.progress.setValue)
        self.move_worker.finished.connect(self._move_finished)
        self.move_worker.finished.connect(self.move_thread.quit)
        self.move_worker.finished.connect(self.move_worker.deleteLater)
        self.move_thread.finished.connect(self.move_thread.deleteLater)
        self.move_thread.start()

    def move_selected_to_staging(self) -> None:
        self._perform_move_to_staging(self._selected_items())

    def move_marked_to_staging(self) -> None:
        marked = [item for item in self.backup_items if item.path in self.staging_marked]
        self._perform_move_to_staging(marked)

    def move_all_to_staging(self) -> None:
        self._perform_move_to_staging(self._filtered_and_sorted_items())

    @Slot(int, int)
    def _move_finished(self, moved: int, errors: int) -> None:
        self._set_ui_busy(False)
        self.progress.setValue(100)
        self._log(f"Da chuyen {moved} file vao staging, loi {errors} file.")
        self.staging_marked.clear()
        self.card_staged.set_value(f"{moved:,}")
        self.start_scan()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("Revit Backup Inspector")
    app.setOrganizationName("DoanVan1725")
    icon = create_app_icon()
    app.setWindowIcon(icon)
    window = MainWindow()
    window.setWindowIcon(icon)
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
