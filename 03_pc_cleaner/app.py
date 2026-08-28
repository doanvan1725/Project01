from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable

from send2trash import send2trash
from PySide6.QtCore import QDateTime, QObject, QThread, Qt, QUrl, Signal, Slot
from PySide6.QtGui import QColor, QDesktopServices, QFont, QIcon, QLinearGradient, QPainter, QPalette, QPixmap, QBrush
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QFileDialog,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
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


@dataclass(frozen=True)
class CleanItem:
    path: Path
    category: str
    size: int
    modified: datetime

    @property
    def modified_display(self) -> str:
        return self.modified.strftime("%Y-%m-%d %H:%M:%S")


def human_size(value: int) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    size = float(value)
    for unit in units:
        if size < 1024 or unit == units[-1]:
            return f"{int(size)} {unit}" if unit == "B" else f"{size:.1f} {unit}"
        size /= 1024
    return f"{value} B"


def runtime_dir() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent


def default_targets() -> list[tuple[str, Path]]:
    local_app_data = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local"))
    targets = [
        ("User Temp", Path(os.environ.get("TEMP", Path.home() / "AppData/Local/Temp"))),
        ("Windows Temp", Path(os.environ.get("SystemRoot", "C:/Windows")) / "Temp"),
        ("Chrome Cache", local_app_data / "Google/Chrome/User Data/Default/Cache"),
        ("Edge Cache", local_app_data / "Microsoft/Edge/User Data/Default/Cache"),
        ("Firefox Cache", local_app_data / "Mozilla/Firefox/Profiles"),
    ]
    return [(label, path) for label, path in targets if path.exists()]


def iter_files(root: Path) -> Iterable[Path]:
    if not root.exists() or not root.is_dir():
        return
    for dirpath, dirs, filenames in os.walk(root, onerror=lambda _: None, followlinks=False):
        dirs[:] = [name for name in dirs if not (Path(dirpath) / name).is_symlink()]
        for filename in filenames:
            path = Path(dirpath) / filename
            if not path.is_symlink():
                yield path


def expand_targets(targets: list[tuple[str, Path]]) -> list[tuple[str, Path]]:
    expanded: list[tuple[str, Path]] = []
    for category, root in targets:
        if root.name == "Profiles" and category == "Firefox Cache":
            for profile in root.glob("*"):
                cache = profile / "cache2"
                if cache.exists():
                    expanded.append((f"{category} - {profile.name}", cache))
        else:
            expanded.append((category, root))
    return expanded


def scan_targets(targets: list[tuple[str, Path]], progress_cb=None, status_cb=None) -> list[CleanItem]:
    items: list[CleanItem] = []
    processed = 0
    for category, root in expand_targets(targets):
        if status_cb:
            status_cb(f"Dang quet {category}...")
        for path in iter_files(root):
            processed += 1
            if progress_cb and processed % 250 == 0:
                progress_cb(min(processed % 100, 99))
            try:
                stat = path.stat()
                items.append(CleanItem(path, category, stat.st_size, datetime.fromtimestamp(stat.st_mtime)))
            except (OSError, PermissionError):
                continue
    items.sort(key=lambda item: (item.size, item.modified), reverse=True)
    return items


class ScanWorker(QObject):
    finished = Signal(object, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(self, targets: list[tuple[str, Path]]) -> None:
        super().__init__()
        self.targets = targets

    @Slot()
    def run(self) -> None:
        try:
            items = scan_targets(self.targets, self.progress.emit, self.status.emit)
            self.finished.emit(items, 0)
        except Exception as exc:  # pragma: no cover - surfaced in UI
            self.status.emit(f"Scan failed: {exc}")
            self.finished.emit([], 1)


class CleanWorker(QObject):
    finished = Signal(int, int)
    progress = Signal(int)
    status = Signal(str)

    def __init__(self, paths: list[Path]) -> None:
        super().__init__()
        self.paths = paths

    @Slot()
    def run(self) -> None:
        moved = 0
        errors = 0
        total = max(1, len(self.paths))
        for index, path in enumerate(self.paths, start=1):
            try:
                send2trash(str(path))
                moved += 1
            except Exception as exc:
                errors += 1
                self.status.emit(f"Khong don duoc {path.name}: {exc}")
            self.progress.emit(int(index * 100 / total))
        self.finished.emit(moved, errors)


class MetricCard(QFrame):
    def __init__(self, title: str, value: str, accent: str) -> None:
        super().__init__()
        self.setObjectName("metricCard")
        self.setMinimumHeight(100)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 15, 18, 15)
        self.title_label = QLabel(title)
        self.title_label.setObjectName("metricTitle")
        self.value_label = QLabel(value)
        self.value_label.setObjectName("metricValue")
        accent_label = QLabel()
        accent_label.setFixedHeight(4)
        accent_label.setStyleSheet(f"background: {accent}; border-radius: 2px;")
        layout.addWidget(self.title_label)
        layout.addWidget(self.value_label)
        layout.addStretch(1)
        layout.addWidget(accent_label)

    def set_value(self, value: str) -> None:
        self.value_label.setText(value)


class MainWindow(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("PC Cleaner")
        self.resize(1380, 900)
        self.targets = default_targets()
        self.items: list[CleanItem] = []
        self.scan_thread: QThread | None = None
        self.scan_worker: ScanWorker | None = None
        self.clean_thread: QThread | None = None
        self.clean_worker: CleanWorker | None = None
        self.log_path = self._init_log_file()
        self._build_ui()
        self._apply_theme()
        self._log(f"PC Cleaner started - log file: {self.log_path}")

    def _init_log_file(self) -> Path:
        folder = runtime_dir() / "logs"
        folder.mkdir(parents=True, exist_ok=True)
        return folder / f"pc_cleaner_{datetime.now():%Y%m%d_%H%M%S}.log"

    def _build_ui(self) -> None:
        root = QWidget()
        self.setCentralWidget(root)
        layout = QVBoxLayout(root)
        layout.setContentsMargins(18, 18, 18, 18)
        layout.setSpacing(14)

        hero = QFrame()
        hero.setObjectName("heroPanel")
        hero_layout = QHBoxLayout(hero)
        hero_layout.setContentsMargins(24, 22, 24, 22)
        badge = QLabel("CC")
        badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        badge.setObjectName("appBadge")
        badge.setFixedSize(70, 70)
        text = QVBoxLayout()
        title = QLabel("PC Cleaner")
        title.setObjectName("heroTitle")
        subtitle = QLabel("Quet, xem truoc va don dep file tam/cache an toan tren Windows.")
        subtitle.setObjectName("heroSubtitle")
        self.path_label = QLabel("Phan tich cac thu muc temp va cache pho bien")
        self.path_label.setObjectName("pathLabel")
        text.addWidget(title)
        text.addWidget(subtitle)
        text.addWidget(self.path_label)
        hero_layout.addWidget(badge)
        hero_layout.addLayout(text, 1)
        layout.addWidget(hero)

        cards = QGridLayout()
        self.card_files = MetricCard("File co the don", "0", "#38bdf8")
        self.card_size = MetricCard("Dung luong co the thu hoi", "0 B", "#22c55e")
        self.card_selected = MetricCard("Dang chon", "0", "#f59e0b")
        self.card_categories = MetricCard("Nhom du lieu", "0", "#a855f7")
        for col, card in enumerate((self.card_files, self.card_size, self.card_selected, self.card_categories)):
            cards.addWidget(card, 0, col)
        layout.addLayout(cards)

        controls = QFrame()
        controls.setObjectName("panel")
        controls_layout = QGridLayout(controls)
        controls_layout.setContentsMargins(18, 15, 18, 15)
        self.scan_button = QPushButton("Quet may")
        self.scan_button.clicked.connect(self.start_scan)
        self.select_all_button = QPushButton("Chon tat ca")
        self.select_all_button.clicked.connect(lambda: self.set_all_checked(True))
        self.clear_button = QPushButton("Bo chon tat ca")
        self.clear_button.clicked.connect(lambda: self.set_all_checked(False))
        self.clean_button = QPushButton("Don file da chon vao Recycle Bin")
        self.clean_button.clicked.connect(self.clean_selected)
        self.open_button = QPushButton("Mo thu muc file dang chon")
        self.open_button.clicked.connect(self.open_selected_folder)
        self.add_folder_button = QPushButton("Them thu muc quet")
        self.add_folder_button.clicked.connect(self.add_scan_folder)
        controls_layout.addWidget(self.scan_button, 0, 0)
        controls_layout.addWidget(self.add_folder_button, 0, 1)
        controls_layout.addWidget(self.select_all_button, 0, 2)
        controls_layout.addWidget(self.clear_button, 0, 3)
        controls_layout.addWidget(self.clean_button, 0, 4)
        controls_layout.addWidget(self.open_button, 0, 5)
        layout.addWidget(controls)

        table_panel = QFrame()
        table_panel.setObjectName("panel")
        table_layout = QVBoxLayout(table_panel)
        table_layout.setContentsMargins(18, 15, 18, 18)
        self.table = QTableWidget(0, 6)
        self.table.setHorizontalHeaderLabels(["Chon", "File", "Nhom", "Kich thuoc", "Cap nhat", "Duong dan"])
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.table.setAlternatingRowColors(True)
        self.table.verticalHeader().setVisible(False)
        self.table.horizontalHeader().setStretchLastSection(True)
        self.table.itemChanged.connect(self._item_changed)
        self.table.itemDoubleClicked.connect(lambda _item: self.open_selected_folder())
        table_layout.addWidget(self.table)
        layout.addWidget(table_panel, 1)

        self.progress = QProgressBar()
        self.progress.setRange(0, 100)
        self.log = QTextEdit()
        self.log.setReadOnly(True)
        self.log.setMaximumHeight(145)
        layout.addWidget(self.progress)
        layout.addWidget(self.log)
        self.statusBar().showMessage("San sang")

    def _apply_theme(self) -> None:
        self.setFont(QFont("Segoe UI", 10))
        palette = QPalette()
        palette.setColor(QPalette.ColorRole.Window, QColor("#0f172a"))
        palette.setColor(QPalette.ColorRole.Base, QColor("#111827"))
        palette.setColor(QPalette.ColorRole.AlternateBase, QColor("#0b1220"))
        palette.setColor(QPalette.ColorRole.Text, QColor("#e5eefb"))
        palette.setColor(QPalette.ColorRole.WindowText, QColor("#e5eefb"))
        palette.setColor(QPalette.ColorRole.Highlight, QColor("#38bdf8"))
        palette.setColor(QPalette.ColorRole.HighlightedText, QColor("#081018"))
        self.setPalette(palette)
        self.setStyleSheet("""
            QWidget { color: #e5eefb; font-family: "Segoe UI"; font-size: 10pt; }
            QMainWindow { background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #08111f, stop:1 #101b33); }
            #heroPanel, #panel, #metricCard { background: rgba(15, 23, 42, 0.9); border: 1px solid rgba(148, 163, 184, 0.18); border-radius: 18px; }
            #appBadge { background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #0ea5e9, stop:1 #22c55e); color: white; border-radius: 18px; font-size: 22pt; font-weight: 800; }
            #heroTitle { font-size: 25pt; font-weight: 700; color: #f8fbff; }
            #heroSubtitle { color: #b5c3da; font-size: 10.5pt; }
            #pathLabel { color: #8ec8ff; padding-top: 4px; }
            #metricTitle { color: #9fb2ce; font-size: 9.5pt; }
            #metricValue { font-size: 21pt; font-weight: 700; color: white; }
            QPushButton { background: #1f3fbf; border: 1px solid rgba(255,255,255,0.12); padding: 10px 12px; border-radius: 12px; font-weight: 600; }
            QPushButton:hover { background: #2d53df; }
            QPushButton:disabled { color: #93a4bd; background: #23314c; }
            QTableWidget, QTextEdit { background: #0b1220; border: 1px solid rgba(148,163,184,0.18); border-radius: 14px; gridline-color: rgba(148,163,184,0.12); selection-background-color: rgba(56,189,248,0.18); selection-color: white; }
            QHeaderView::section { background: #12203b; color: #d6e3fb; padding: 10px 8px; border: 0; font-weight: 600; }
            QProgressBar { border: 1px solid rgba(148,163,184,0.22); border-radius: 8px; text-align: center; background: #0b1324; height: 18px; }
            QProgressBar::chunk { border-radius: 8px; background: qlineargradient(x1:0,y1:0,x2:1,y2:0,stop:0 #0ea5e9,stop:1 #22c55e); }
        """)

    def _log(self, message: str) -> None:
        line = f"[{QDateTime.currentDateTime().toString('HH:mm:ss')}] {message}"
        self.log.append(line)
        self.statusBar().showMessage(message)
        try:
            with self.log_path.open("a", encoding="utf-8") as handle:
                handle.write(line + "\n")
        except OSError:
            pass

    def _set_busy(self, busy: bool) -> None:
        for button in (self.scan_button, self.add_folder_button, self.select_all_button, self.clear_button, self.clean_button):
            button.setDisabled(busy)

    def add_scan_folder(self) -> None:
        folder = QFileDialog.getExistingDirectory(self, "Chon thu muc bo sung")
        if folder:
            self.targets.append((f"Thu muc them - {Path(folder).name}", Path(folder)))
            self.path_label.setText(f"Da them: {folder}")
            self._log(f"Them thu muc quet: {folder}")

    def start_scan(self) -> None:
        self._set_busy(True)
        self.progress.setValue(0)
        self._log("Bat dau quet cac thu muc temp/cache...")
        self.scan_thread = QThread(self)
        self.scan_worker = ScanWorker(self.targets)
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
        self._set_busy(False)
        self.items = items if error_code == 0 else []
        self._refresh_table()
        self.progress.setValue(100 if error_code == 0 else 0)
        self._log(f"Quet xong: {len(self.items):,} file co the don.")

    def _refresh_table(self) -> None:
        self.table.blockSignals(True)
        self.table.setRowCount(len(self.items))
        for row, item in enumerate(self.items):
            check = QTableWidgetItem()
            check.setCheckState(Qt.CheckState.Unchecked)
            values = ["", item.path.name, item.category, human_size(item.size), item.modified_display, str(item.path)]
            self.table.setItem(row, 0, check)
            for col, value in enumerate(values[1:], start=1):
                table_item = QTableWidgetItem(value)
                if col == 3:
                    table_item.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                self.table.setItem(row, col, table_item)
        self.table.blockSignals(False)
        self.table.resizeColumnsToContents()
        self.table.setColumnWidth(0, 55)
        self.table.setColumnWidth(1, max(220, self.table.columnWidth(1)))
        self.table.setColumnWidth(5, max(360, self.table.columnWidth(5)))
        self.card_files.set_value(f"{len(self.items):,}")
        self.card_size.set_value(human_size(sum(item.size for item in self.items)))
        self.card_categories.set_value(f"{len({item.category for item in self.items}):,}")
        self._update_selected_card()

    def _item_changed(self, item: QTableWidgetItem) -> None:
        if item.column() == 0:
            checked = item.checkState() == Qt.CheckState.Checked
            color = QColor("#713f12") if checked else QColor("#0b1220")
            for col in range(self.table.columnCount()):
                cell = self.table.item(item.row(), col)
                if cell:
                    cell.setBackground(QBrush(color) if checked else QBrush())
            self._update_selected_card()

    def _update_selected_card(self) -> None:
        count = len(self.checked_items())
        self.card_selected.set_value(f"{count:,}")

    def checked_items(self) -> list[CleanItem]:
        return [item for row, item in enumerate(self.items) if self.table.item(row, 0).checkState() == Qt.CheckState.Checked]

    def set_all_checked(self, checked: bool) -> None:
        state = Qt.CheckState.Checked if checked else Qt.CheckState.Unchecked
        for row in range(self.table.rowCount()):
            self.table.item(row, 0).setCheckState(state)

    def open_selected_folder(self) -> None:
        rows = self.table.selectionModel().selectedRows()
        if not rows:
            return
        path = self.items[rows[0].row()].path.parent
        if path.exists():
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(path)))

    def clean_selected(self) -> None:
        items = self.checked_items()
        if not items:
            QMessageBox.information(self, "Chua chon", "Hay chon file can don truoc.")
            return
        total = sum(item.size for item in items)
        answer = QMessageBox.warning(
            self,
            "Xac nhan don dep",
            f"Dua {len(items):,} file ({human_size(total)}) vao Recycle Bin?\n\nBan co the khoi phuc tu Recycle Bin neu can.",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if answer != QMessageBox.StandardButton.Yes:
            return
        self._set_busy(True)
        self.progress.setValue(0)
        self._log(f"Bat dau dua {len(items)} file vao Recycle Bin...")
        self.clean_thread = QThread(self)
        self.clean_worker = CleanWorker([item.path for item in items])
        self.clean_worker.moveToThread(self.clean_thread)
        self.clean_thread.started.connect(self.clean_worker.run)
        self.clean_worker.status.connect(self._log)
        self.clean_worker.progress.connect(self.progress.setValue)
        self.clean_worker.finished.connect(self._clean_finished)
        self.clean_worker.finished.connect(self.clean_thread.quit)
        self.clean_worker.finished.connect(self.clean_worker.deleteLater)
        self.clean_thread.finished.connect(self.clean_thread.deleteLater)
        self.clean_thread.start()

    @Slot(int, int)
    def _clean_finished(self, moved: int, errors: int) -> None:
        self._set_busy(False)
        self._log(f"Da dua {moved} file vao Recycle Bin, loi {errors} file.")
        self.start_scan()


def create_app_icon() -> QIcon:
    pixmap = QPixmap(256, 256)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    gradient = QLinearGradient(0, 0, 256, 256)
    gradient.setColorAt(0.0, QColor("#0ea5e9"))
    gradient.setColorAt(1.0, QColor("#16a34a"))
    painter.setBrush(gradient)
    painter.setPen(Qt.PenStyle.NoPen)
    painter.drawRoundedRect(12, 12, 232, 232, 48, 48)
    painter.setPen(QColor("white"))
    painter.setFont(QFont("Segoe UI", 86, QFont.Weight.Bold))
    painter.drawText(pixmap.rect(), Qt.AlignmentFlag.AlignCenter, "CC")
    painter.end()
    return QIcon(pixmap)


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("PC Cleaner")
    app.setOrganizationName("DoanVan1725")
    icon = create_app_icon()
    app.setWindowIcon(icon)
    window = MainWindow()
    window.setWindowIcon(icon)
    window.show()
    return app.exec()
