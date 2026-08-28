import { useEffect, useMemo, useState, type ChangeEvent, type FormEvent, type ReactNode } from "react";
import {
  Archive,
  ArrowDownToLine,
  Bell,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  ClipboardList,
  CloudUpload,
  ExternalLink,
  FileArchive,
  FileImage,
  FileSpreadsheet,
  FileText,
  Filter,
  FolderOpen,
  LayoutDashboard,
  Link as LinkIcon,
  Loader2,
  LogOut,
  Menu,
  MessageSquareText,
  MoreHorizontal,
  Plus,
  Search,
  Settings,
  ShieldCheck,
  Trash2,
  Upload,
  UserRound,
  UsersRound,
  X,
} from "lucide-react";
import { answerIssue, createIssue, fetchIssues, getCurrentProfile, removeIssue, signIn, signUp, uploadAttachment } from "./lib/issues";
import { isSupabaseConfigured, supabase } from "./lib/supabase";
import { mockIssues } from "./mockData";
import type { Attachment, Issue, IssueStatus, UserRole } from "./types";
import { STATUS_OPTIONS } from "./types";

const categoryOptions = ["Kiến trúc", "Kết cấu", "MEP", "Hạ tầng", "Biện pháp thi công"];

const roleLabel: Record<UserRole, string> = { admin: "Quản trị viên", editor: "Biên tập viên", viewer: "Người xem" };

function formatDate(value: string | null) {
  if (!value) return "-";
  return new Intl.DateTimeFormat("vi-VN", { hour: "2-digit", minute: "2-digit", second: "2-digit", day: "2-digit", month: "2-digit", year: "numeric" }).format(new Date(value)).replace(",", " -");
}

function fileIcon(attachment: Attachment) {
  if (attachment.kind === "link") return <LinkIcon size={15} />;
  const extension = attachment.name.split(".").pop()?.toLowerCase();
  if (extension === "pdf" || extension === "doc" || extension === "docx") return <FileText size={15} />;
  if (["png", "jpg", "jpeg", "webp"].includes(extension ?? "")) return <FileImage size={15} />;
  if (["xls", "xlsx", "csv"].includes(extension ?? "")) return <FileSpreadsheet size={15} />;
  if (["zip", "rar", "7z"].includes(extension ?? "")) return <FileArchive size={15} />;
  return <Archive size={15} />;
}

function attachmentClass(attachment: Attachment) {
  if (attachment.kind === "link") return "attachment attachment-link";
  const extension = attachment.name.split(".").pop()?.toLowerCase();
  if (extension === "pdf") return "attachment attachment-pdf";
  if (["doc", "docx"].includes(extension ?? "")) return "attachment attachment-doc";
  if (["xls", "xlsx", "csv"].includes(extension ?? "")) return "attachment attachment-xls";
  if (["png", "jpg", "jpeg", "webp"].includes(extension ?? "")) return "attachment attachment-img";
  return "attachment attachment-other";
}

function StatusBadge({ status }: { status: IssueStatus }) {
  const styles: Record<IssueStatus, string> = { "Mới tạo": "status-new", "Đang xử lý": "status-progress", "Đã trả lời": "status-done" };
  return <span className={`status-badge ${styles[status]} `}><span className="status-dot" />{status}</span>;
}

function Modal({ title, onClose, children, wide = false }: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <div className={`modal-card ${wide ? "modal-wide" : ""}`} role="dialog" aria-modal="true">
      <div className="modal-header"><div><p className="eyebrow">ISSUE TRACKER</p><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Đóng"><X size={20} /></button></div>
      {children}
    </div>
  </div>;
}

function App() {
  const [issues, setIssues] = useState<Issue[]>(mockIssues);
  const [loading, setLoading] = useState(isSupabaseConfigured);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("Tất cả hạng mục");
  const [status, setStatus] = useState("Tất cả trạng thái");
  const [pageSize, setPageSize] = useState(10);
  const [page, setPage] = useState(1);
  const [activeModal, setActiveModal] = useState<"create" | "answer" | "view" | null>(null);
  const [answerTarget, setAnswerTarget] = useState<Issue | null>(null);
  const [viewTarget, setViewTarget] = useState<Issue | null>(null);
  const [busy, setBusy] = useState(false);
  const [role, setRole] = useState<UserRole>("admin");
  const [profile, setProfile] = useState<{ email: string; fullName: string; role: UserRole } | null>(null);
  const [authReady, setAuthReady] = useState(!isSupabaseConfigured);
  const [mobileNav, setMobileNav] = useState(false);

  useEffect(() => {
    if (!supabase) return;
    let mounted = true;
    const loadProfile = async () => {
      try {
        const current = await getCurrentProfile();
        if (mounted) { setProfile(current); if (current) setRole(current.role); }
      } catch (reason) {
        if (mounted) setError(reason instanceof Error ? reason.message : "Không thể tải hồ sơ người dùng");
      } finally {
        if (mounted) setAuthReady(true);
      }
    };
    void loadProfile();
    const { data } = supabase.auth.onAuthStateChange(() => { void loadProfile(); });
    return () => { mounted = false; data.subscription.unsubscribe(); };
  }, []);

  useEffect(() => {
    if (!supabase || !profile) return;
    fetchIssues().then(setIssues).catch((reason: Error) => setError(reason.message)).finally(() => setLoading(false));
  }, [profile]);

  const canCreate = role === "admin" || role === "editor";
  const canEdit = canCreate;
  const canDelete = role === "admin";
  const filteredIssues = useMemo(() => issues.filter((issue) => {
    const haystack = `${issue.creatorName} ${issue.category} ${issue.content} ${issue.reply}`.toLowerCase();
    return (!query || haystack.includes(query.toLowerCase())) && (category === "Tất cả hạng mục" || issue.category === category) && (status === "Tất cả trạng thái" || issue.status === status);
  }), [issues, query, category, status]);
  const totalPages = Math.max(1, Math.ceil(filteredIssues.length / pageSize));
  const visibleIssues = filteredIssues.slice((page - 1) * pageSize, page * pageSize);
  const stats = { total: issues.length, open: issues.filter((issue) => issue.status !== "Đã trả lời").length, answered: issues.filter((issue) => issue.status === "Đã trả lời").length };

  useEffect(() => setPage((current) => Math.min(current, totalPages)), [totalPages]);

  async function reload() {
    if (!supabase) return;
    setIssues(await fetchIssues());
  }

  async function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const file = form.get("file") as File | null;
    setBusy(true); setError("");
    try {
      const attachments: Attachment[] = [];
      const link = String(form.get("link") ?? "").trim();
      if (file && file.size > 0) attachments.push(await uploadAttachment(file));
      if (link) attachments.push({ name: link.replace(/^https?:\/\//, ""), url: link, kind: "link" });
      const input = { creatorName: String(form.get("creatorName")), category: String(form.get("category")), content: String(form.get("content")), attachments };
      if (supabase) await createIssue(input);
      else setIssues((current) => [{ ...input, id: crypto.randomUUID(), createdAt: new Date().toISOString(), reply: "", responderName: "", repliedAt: null, status: "Mới tạo" }, ...current]);
      setActiveModal(null); setPage(1); await reload();
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Không thể tạo issue"); }
    finally { setBusy(false); }
  }

  async function handleAnswer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!answerTarget) return;
    const form = new FormData(event.currentTarget);
    const responderName = String(form.get("responderName"));
    const reply = String(form.get("reply"));
    setBusy(true); setError("");
    try {
      if (supabase) await answerIssue(answerTarget.id, responderName, reply);
      else setIssues((current) => current.map((issue) => issue.id === answerTarget.id ? { ...issue, responderName, reply, repliedAt: new Date().toISOString(), status: "Đã trả lời" } : issue));
      setActiveModal(null); setAnswerTarget(null); await reload();
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Không thể gửi trả lời"); }
    finally { setBusy(false); }
  }

  async function handleDelete(issue: Issue) {
    if (!canDelete || !window.confirm(`Xóa issue của ${issue.creatorName}? Hành động này không thể hoàn tác.`)) return;
    setBusy(true); setError("");
    try { if (supabase) await removeIssue(issue.id); else setIssues((current) => current.filter((item) => item.id !== issue.id)); await reload(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Không thể xóa issue"); }
    finally { setBusy(false); }
  }

  if (isSupabaseConfigured && !authReady) return <div className="auth-loading"><Loader2 className="spin" size={24} /> Đang kiểm tra phiên đăng nhập...</div>;
  if (isSupabaseConfigured && !profile) return <AuthScreen />;

  return <div className="app-shell">
    <aside className={`sidebar ${mobileNav ? "sidebar-open" : ""}`}>
      <div className="brand"><div className="brand-mark"><ClipboardList size={22} /></div><div><strong>ISSUE<span>TRACKER</span></strong><small>BIM PROJECT CONTROL</small></div></div>
      <nav className="main-nav">
        <p className="nav-label">TỔNG QUAN</p>
        <button className="nav-item active"><LayoutDashboard size={18} /> Bảng theo dõi</button>
        <button className="nav-item"><UsersRound size={18} /> Thành viên</button>
        <button className="nav-item"><FolderOpen size={18} /> Thư viện hồ sơ</button>
        <p className="nav-label nav-gap">HỆ THỐNG</p>
        <button className="nav-item"><ShieldCheck size={18} /> Phân quyền</button>
        <button className="nav-item"><Settings size={18} /> Cài đặt</button>
      </nav>
      <div className="sidebar-footer"><div className="support-card"><CircleHelp size={19} /><div><strong>Cần hỗ trợ?</strong><span>Liên hệ quản trị viên</span></div></div><button className="nav-item logout" onClick={() => { if (supabase) void supabase.auth.signOut(); }}><LogOut size={18} /> Đăng xuất</button></div>
    </aside>
    {mobileNav && <button className="mobile-overlay" onClick={() => setMobileNav(false)} aria-label="Đóng menu" />}
    <main className="main-content">
      <header className="topbar"><button className="mobile-menu icon-button" onClick={() => setMobileNav(true)}><Menu size={21} /></button><div className="breadcrumb"><span>DỰ ÁN</span><ChevronRight size={14} /><strong>ISSUE TRACKER</strong></div><div className="top-actions"><div className="role-switch"><UserRound size={15} />{supabase ? <><span>{profile?.fullName || profile?.email}</span><span className="role-pill">{roleLabel[role]}</span></> : <><select value={role} onChange={(event) => setRole(event.target.value as UserRole)} aria-label="Vai trò demo"><option value="admin">Admin</option><option value="editor">Editor</option><option value="viewer">Viewer</option></select><ChevronDown size={13} /></>}</div><button className="notification icon-button"><Bell size={19} /><i /></button><div className="avatar">{profile?.fullName?.slice(0, 2).toUpperCase() || "NV"}</div></div></header>
      <section className="page-heading"><div><p className="eyebrow blue">QUẢN LÝ PHỐI HỢP DỰ ÁN</p><h1>Bảng theo dõi ý kiến <span>(Issue)</span></h1><p className="heading-note">Theo dõi, phản hồi và lưu trữ toàn bộ ý kiến trong một không gian thống nhất.</p></div>{canCreate && <button className="primary-button" onClick={() => setActiveModal("create")}><Plus size={18} /> Tạo Issue mới</button>}</section>
      {error && <div className="alert-error"><X size={16} /> {error}</div>}
      <section className="stats-grid"><div className="stat-card"><div className="stat-icon stat-blue"><ClipboardList size={19} /></div><div><span>Tổng số Issue</span><strong>{stats.total}</strong></div><small className="trend">+12% <em>so với tháng trước</em></small></div><div className="stat-card"><div className="stat-icon stat-orange"><MessageSquareText size={19} /></div><div><span>Đang xử lý</span><strong>{stats.open}</strong></div><small className="trend orange">Cần phản hồi</small></div><div className="stat-card"><div className="stat-icon stat-green"><Check size={19} /></div><div><span>Đã trả lời</span><strong>{stats.answered}</strong></div><small className="trend green">Đã hoàn tất</small></div></section>
      <section className="table-card"><div className="table-toolbar"><div><h2>Danh sách Issue</h2><p>{loading ? "Đang đồng bộ dữ liệu..." : `${filteredIssues.length} issue trong hệ thống`}</p></div><div className="toolbar-actions"><label className="search-box"><Search size={17} /><input value={query} onChange={(event) => { setQuery(event.target.value); setPage(1); }} placeholder="Tìm theo nội dung, người tạo..." /></label><div className="filter-select"><Filter size={15} /><select value={category} onChange={(event) => { setCategory(event.target.value); setPage(1); }}><option>Tất cả hạng mục</option>{categoryOptions.map((item) => <option key={item}>{item}</option>)}</select><ChevronDown size={14} /></div><div className="filter-select"><select value={status} onChange={(event) => { setStatus(event.target.value); setPage(1); }}><option>Tất cả trạng thái</option>{STATUS_OPTIONS.map((item) => <option key={item}>{item}</option>)}</select><ChevronDown size={14} /></div></div></div>
        <div className="table-scroll"><table><thead><tr><th className="index-col">STT <ChevronDown size={12} /></th><th>Người tạo</th><th>Hạng mục</th><th className="content-col">Nội dung</th><th>Đính kèm</th><th>Ngày tạo</th><th>Trả lời</th><th>Người trả lời</th><th>Ngày trả lời</th><th>Trạng thái</th><th /></tr></thead><tbody>{loading ? <tr><td colSpan={11} className="empty-state"><Loader2 className="spin" /> Đang tải dữ liệu...</td></tr> : visibleIssues.length === 0 ? <tr><td colSpan={11} className="empty-state">Không tìm thấy issue phù hợp.</td></tr> : visibleIssues.map((issue, index) => <tr key={issue.id}><td className="index-col">{(page - 1) * pageSize + index + 1}</td><td><div className="creator-cell"><div className="mini-avatar">{issue.creatorName.split(" ").map((part) => part[0]).slice(-2).join("")}</div><span>{issue.creatorName}</span></div></td><td><span className="category-text">{issue.category}</span></td><td className="content-cell"><span>{issue.content.length > 92 ? `${issue.content.slice(0, 92)}...` : issue.content}</span>{issue.content.length > 92 && <button className="more-link" onClick={() => { setViewTarget(issue); setActiveModal("view"); }}>Xem thêm</button>}</td><td><div className="attachments">{issue.attachments.length ? issue.attachments.map((attachment) => <a key={`${issue.id}-${attachment.name}`} className={attachmentClass(attachment)} href={attachment.url} target="_blank" rel="noreferrer" title={attachment.name}>{fileIcon(attachment)}<span>{attachment.name.length > 13 ? `${attachment.name.slice(0, 10)}...` : attachment.name}</span><ExternalLink size={11} /></a>) : <span className="no-file">-</span>}</div></td><td className="date-cell">{formatDate(issue.createdAt)}</td><td className="reply-cell">{issue.reply ? issue.reply.length > 55 ? `${issue.reply.slice(0, 55)}...` : issue.reply : <span className="muted">Chưa có phản hồi</span>}</td><td>{issue.responderName || <span className="muted">-</span>}</td><td className="date-cell">{formatDate(issue.repliedAt)}</td><td><StatusBadge status={issue.status} /></td><td><div className="row-actions">{canEdit && <button className="row-icon" title="Trả lời" onClick={() => { setAnswerTarget(issue); setActiveModal("answer"); }}><MessageSquareText size={16} /></button>}{canDelete && <button className="row-icon danger" title="Xóa" onClick={() => handleDelete(issue)}><Trash2 size={16} /></button>}<MoreHorizontal size={17} className="muted" /></div></td></tr>)}</tbody></table></div>
        <div className="table-footer"><span>Hiển thị {visibleIssues.length ? (page - 1) * pageSize + 1 : 0}-{Math.min(page * pageSize, filteredIssues.length)} trên {filteredIssues.length}</span><div className="pagination"><label>Hiển thị <select value={pageSize} onChange={(event) => { setPageSize(Number(event.target.value)); setPage(1); }}><option value={10}>10</option><option value={20}>20</option></select> / trang</label><button className="page-button" disabled={page <= 1} onClick={() => setPage((current) => current - 1)}><ChevronLeft size={16} /></button><span className="page-current">{page}</span><button className="page-button" disabled={page >= totalPages} onClick={() => setPage((current) => current + 1)}><ChevronRight size={16} /></button></div></div>
      </section>
      <footer className="page-footer"><span><CloudUpload size={14} /> Dữ liệu được đồng bộ an toàn</span><span>{isSupabaseConfigured ? "Supabase Cloud" : "Demo Mock Data"} · Cập nhật lúc {new Date().toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit" })}</span></footer>
    </main>
    {activeModal === "create" && <CreateModal busy={busy} onClose={() => setActiveModal(null)} onSubmit={handleCreate} />}
    {activeModal === "answer" && answerTarget && <AnswerModal issue={answerTarget} busy={busy} onClose={() => { setActiveModal(null); setAnswerTarget(null); }} onSubmit={handleAnswer} />}
    {activeModal === "view" && viewTarget && <Modal title="Chi tiết nội dung Issue" onClose={() => { setActiveModal(null); setViewTarget(null); }} wide><div className="detail-meta"><StatusBadge status={viewTarget.status} /><span>{formatDate(viewTarget.createdAt)}</span><span>{viewTarget.category}</span></div><div className="detail-content">{viewTarget.content}</div>{viewTarget.attachments.length > 0 && <div className="detail-files"><h3>Hồ sơ đính kèm</h3>{viewTarget.attachments.map((attachment) => <a key={attachment.name} href={attachment.url} target="_blank" rel="noreferrer" className="detail-file">{fileIcon(attachment)} {attachment.name}<ArrowDownToLine size={15} /></a>)}</div>}</Modal>}
  </div>;
}

function AuthScreen() {
  const [mode, setMode] = useState<"login" | "signup">("login");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    setBusy(true); setError(""); setMessage("");
    try {
      const email = String(form.get("email"));
      const password = String(form.get("password"));
      if (mode === "signup") {
        await signUp(email, password, String(form.get("fullName")));
        setMessage("Tài khoản đã tạo. Hãy kiểm tra email nếu Supabase yêu cầu xác nhận.");
      } else {
        await signIn(email, password);
      }
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Đăng nhập thất bại"); }
    finally { setBusy(false); }
  }

  return <div className="auth-screen"><div className="auth-card"><div className="auth-brand"><div className="brand-mark"><ClipboardList size={22} /></div><div><strong>ISSUE<span>TRACKER</span></strong><small>BIM PROJECT CONTROL</small></div></div><p className="eyebrow blue">KHÔNG GIAN LÀM VIỆC</p><h1>{mode === "login" ? "Chào mừng trở lại" : "Tạo tài khoản"}</h1><p className="auth-note">Đăng nhập để quản lý issue, hồ sơ và quyền truy cập dự án.</p>{error && <div className="alert-error auth-alert"><X size={16} /> {error}</div>}{message && <div className="auth-success"><Check size={16} /> {message}</div>}<form className="issue-form auth-form" onSubmit={submit}>{mode === "signup" && <label>Họ và tên <span>*</span><input name="fullName" required placeholder="Nguyễn Văn A" /></label>}<label>Email <span>*</span><input name="email" type="email" required placeholder="you@company.com" /></label><label>Mật khẩu <span>*</span><input name="password" type="password" minLength={6} required placeholder="Tối thiểu 6 ký tự" /></label><button className="primary-button" disabled={busy} type="submit">{busy ? <Loader2 className="spin" size={17} /> : <ShieldCheck size={17} />} {mode === "login" ? "Đăng nhập" : "Đăng ký"}</button></form><button className="auth-toggle" onClick={() => { setMode(mode === "login" ? "signup" : "login"); setError(""); setMessage(""); }}>{mode === "login" ? "Chưa có tài khoản? Đăng ký" : "Đã có tài khoản? Đăng nhập"}</button></div></div>;
}

function CreateModal({ onClose, onSubmit, busy }: { onClose: () => void; onSubmit: (event: FormEvent<HTMLFormElement>) => void; busy: boolean }) {
  const [fileName, setFileName] = useState("");
  return <Modal title="Tạo Issue mới" onClose={onClose}><form className="issue-form" onSubmit={onSubmit}><div className="form-grid"><label>Người tạo <span>*</span><input name="creatorName" required placeholder="Nhập họ và tên" /></label><label>Hạng mục <span>*</span><select name="category" required defaultValue=""><option value="" disabled>Chọn hạng mục</option>{categoryOptions.map((item) => <option key={item}>{item}</option>)}</select></label></div><label>Nội dung ý kiến <span>*</span><textarea name="content" required rows={6} placeholder="Mô tả chi tiết ý kiến hoặc sự cố..." /><small>Thời gian tạo sẽ được ghi nhận tự động khi gửi.</small></label><div className="form-grid"><label className="file-drop"><span>Đính kèm tệp</span><input name="file" type="file" accept=".pdf,.doc,.docx,.xls,.xlsx,.png,.jpg,.jpeg,.zip,.dwg" onChange={(event: ChangeEvent<HTMLInputElement>) => setFileName(event.target.files?.[0]?.name ?? "")} /><span className="file-drop-box"><Upload size={18} />{fileName || "Kéo thả hoặc bấm để chọn file"}</span><small>PDF, DOCX, XLSX, PNG, JPG, ZIP, DWG...</small></label><label>Hoặc dán đường link<input name="link" type="url" placeholder="https://..." /><small>Link hồ sơ trên Drive, SharePoint...</small></label></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>Hủy</button><button className="primary-button" disabled={busy} type="submit">{busy ? <Loader2 className="spin" size={17} /> : <Plus size={17} />} Tạo Issue</button></div></form></Modal>;
}

function AnswerModal({ issue, onClose, onSubmit, busy }: { issue: Issue; onClose: () => void; onSubmit: (event: FormEvent<HTMLFormElement>) => void; busy: boolean }) {
  return <Modal title="Trả lời Issue" onClose={onClose}><div className="quoted-issue"><span>{issue.category}</span><p>{issue.content}</p><small>Người tạo: {issue.creatorName} · {formatDate(issue.createdAt)}</small></div><form className="issue-form" onSubmit={onSubmit}><label>Người trả lời <span>*</span><input name="responderName" required defaultValue="" placeholder="Nhập họ và tên người trả lời" /></label><label>Nội dung phản hồi <span>*</span><textarea name="reply" required rows={5} defaultValue={issue.reply} placeholder="Nhập nội dung trả lời..." /><small>Thời gian trả lời sẽ được ghi nhận tự động khi gửi.</small></label><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>Hủy</button><button className="primary-button" disabled={busy} type="submit">{busy ? <Loader2 className="spin" size={17} /> : <Check size={17} />} Gửi phản hồi</button></div></form></Modal>;
}

export default App;
