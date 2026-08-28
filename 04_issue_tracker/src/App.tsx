import {
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type FormEvent,
  type ReactNode,
} from "react";
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
  Pencil,
  Plus,
  Search,
  Settings,
  ShieldCheck,
  Trash2,
  Upload,
  Download,
  UserRound,
  UsersRound,
  X,
} from "lucide-react";
import {
  addIssueVersion,
  answerIssue,
  createIssue,
  downloadAttachment,
  fetchIssueVersions,
  fetchProfiles,
  fetchIssues,
  getCurrentProfile,
  removeIssue,
  signIn,
  signOut,
  signUp,
  updateIssue,
  updateUserPermission,
  updateUserRole,
  uploadAttachment,
} from "./lib/issues";
import { isSupabaseConfigured, supabase } from "./lib/supabase";
import { mockIssues } from "./mockData";
import type {
  Attachment,
  Issue,
  IssueStatus,
  IssueVersion,
  UserRole,
} from "./types";
import type { UserProfile } from "./lib/issues";
import { STATUS_OPTIONS } from "./types";

const categoryOptions = [
  "Kiến trúc",
  "Kết cấu",
  "MEP",
  "Hạ tầng",
  "Biện pháp thi công",
];

const roleLabel: Record<UserRole, string> = {
  admin: "Quản trị viên",
  editor: "Biên tập viên",
  viewer: "Người xem",
};

function formatDate(value: string | null) {
  if (!value) return "-";
  return new Intl.DateTimeFormat("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  })
    .format(new Date(value))
    .replace(",", " -");
}

function fileIcon(attachment: Attachment) {
  if (attachment.kind === "link") return <LinkIcon size={15} />;
  const extension = attachment.name.split(".").pop()?.toLowerCase();
  if (extension === "pdf" || extension === "doc" || extension === "docx")
    return <FileText size={15} />;
  if (["png", "jpg", "jpeg", "webp"].includes(extension ?? ""))
    return <FileImage size={15} />;
  if (["xls", "xlsx", "csv"].includes(extension ?? ""))
    return <FileSpreadsheet size={15} />;
  if (["zip", "rar", "7z"].includes(extension ?? ""))
    return <FileArchive size={15} />;
  return <Archive size={15} />;
}

function attachmentClass(attachment: Attachment) {
  if (attachment.kind === "link") return "attachment attachment-link";
  const extension = attachment.name.split(".").pop()?.toLowerCase();
  if (extension === "pdf") return "attachment attachment-pdf";
  if (["doc", "docx"].includes(extension ?? ""))
    return "attachment attachment-doc";
  if (["xls", "xlsx", "csv"].includes(extension ?? ""))
    return "attachment attachment-xls";
  if (["png", "jpg", "jpeg", "webp"].includes(extension ?? ""))
    return "attachment attachment-img";
  return "attachment attachment-other";
}

function StatusBadge({ status }: { status: IssueStatus }) {
  const styles: Record<IssueStatus, string> = {
    "Mới tạo": "status-new",
    "Đang xử lý": "status-progress",
    "Đã trả lời": "status-done",
  };
  return (
    <span className={`status-badge ${styles[status]} `}>
      <span className="status-dot" />
      {status}
    </span>
  );
}

function Modal({
  title,
  onClose,
  children,
  wide = false,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={`modal-card ${wide ? "modal-wide" : ""}`}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-header">
          <div>
            <p className="eyebrow">VCI.SBGB.ISSUE</p>
            <h2>{title}</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Đóng">
            <X size={20} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
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
  const [activeModal, setActiveModal] = useState<
    "create" | "answer" | "view" | "edit" | "version" | "permissions" | null
  >(null);
  const [answerTarget, setAnswerTarget] = useState<Issue | null>(null);
  const [viewTarget, setViewTarget] = useState<Issue | null>(null);
  const [editTarget, setEditTarget] = useState<Issue | null>(null);
  const [versionTarget, setVersionTarget] = useState<Issue | null>(null);
  const [versions, setVersions] = useState<IssueVersion[]>([]);
  const [menuTarget, setMenuTarget] = useState<string | null>(null);
  const [users, setUsers] = useState<UserProfile[]>([]);
  const [busy, setBusy] = useState(false);
  const [role, setRole] = useState<UserRole>("admin");
  const [profile, setProfile] = useState<{
    email: string;
    fullName: string;
    role: UserRole;
    canView: boolean;
    canEdit: boolean;
    canDelete: boolean;
    canDownload: boolean;
  } | null>(null);
  const [authReady, setAuthReady] = useState(!isSupabaseConfigured);
  const [mobileNav, setMobileNav] = useState(false);
  const [activeSection, setActiveSection] = useState<"dashboard" | "members" | "library">("dashboard");

  useEffect(() => {
    if (!supabase) return;
    let mounted = true;
    const loadProfile = async () => {
      try {
        const current = await getCurrentProfile();
        if (mounted) {
          setProfile(current);
          if (current) setRole(current.role);
        }
      } catch (reason) {
        if (mounted)
          setError(
            reason instanceof Error
              ? reason.message
              : "Không thể tải hồ sơ người dùng",
          );
      } finally {
        if (mounted) setAuthReady(true);
      }
    };
    void loadProfile();
    const { data } = supabase.auth.onAuthStateChange(() => {
      void loadProfile();
    });
    return () => {
      mounted = false;
      data.subscription.unsubscribe();
    };
  }, []);

  useEffect(() => {
    if (!supabase || !profile) return;
    fetchIssues()
      .then(setIssues)
      .catch((reason: Error) => setError(reason.message))
      .finally(() => setLoading(false));
  }, [profile]);

  const canView = !supabase || profile?.canView !== false;
  const canCreate = !supabase ? role !== "viewer" : profile?.canEdit === true;
  const canEdit = canCreate;
  const canDelete = !supabase ? role === "admin" : profile?.canDelete === true;
  const canDownload = !supabase || profile?.canDownload === true;
  const filteredIssues = useMemo(
    () =>
      issues.filter((issue) => {
        const haystack =
          `${issue.creatorName} ${issue.category} ${issue.content} ${issue.reply}`.toLowerCase();
        return (
          (!query || haystack.includes(query.toLowerCase())) &&
          (category === "Tất cả hạng mục" || issue.category === category) &&
          (status === "Tất cả trạng thái" || issue.status === status)
        );
      }),
    [issues, query, category, status],
  );
  const totalPages = Math.max(1, Math.ceil(filteredIssues.length / pageSize));
  const visibleIssues = filteredIssues.slice(
    (page - 1) * pageSize,
    page * pageSize,
  );
  const stats = {
    total: issues.length,
    open: issues.filter((issue) => issue.status !== "Đã trả lời").length,
    answered: issues.filter((issue) => issue.status === "Đã trả lời").length,
  };

  useEffect(
    () => setPage((current) => Math.min(current, totalPages)),
    [totalPages],
  );

  async function reload() {
    if (!supabase) return;
    setIssues(await fetchIssues());
  }

  async function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const file = form.get("file") as File | null;
    setBusy(true);
    setError("");
    try {
      const attachments: Attachment[] = [];
      const link = String(form.get("link") ?? "").trim();
      if (file && file.size > 0) attachments.push(await uploadAttachment(file));
      if (link)
        attachments.push({
          name: link.replace(/^https?:\/\//, ""),
          url: link,
          kind: "link",
        });
      const input = {
        creatorName: String(form.get("creatorName")),
        category: String(form.get("category")),
        content: String(form.get("content")),
        attachments,
      };
      if (supabase) await createIssue(input);
      else
        setIssues((current) => [
          {
            ...input,
            id: crypto.randomUUID(),
            createdAt: new Date().toISOString(),
            reply: "",
            responderName: "",
            repliedAt: null,
            status: "Mới tạo",
          },
          ...current,
        ]);
      setActiveModal(null);
      setPage(1);
      await reload();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Không thể tạo issue",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleAnswer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!answerTarget) return;
    const form = new FormData(event.currentTarget);
    const responderName = String(form.get("responderName"));
    const reply = String(form.get("reply"));
    setBusy(true);
    setError("");
    try {
      if (supabase) await answerIssue(answerTarget.id, responderName, reply);
      else
        setIssues((current) =>
          current.map((issue) =>
            issue.id === answerTarget.id
              ? {
                  ...issue,
                  responderName,
                  reply,
                  repliedAt: new Date().toISOString(),
                  status: "Đã trả lời",
                }
              : issue,
          ),
        );
      setActiveModal(null);
      setAnswerTarget(null);
      await reload();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Không thể gửi trả lời",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(issue: Issue) {
    if (
      !canDelete ||
      !window.confirm(
        `Xóa issue của ${issue.creatorName}? Hành động này không thể hoàn tác.`,
      )
    )
      return;
    setBusy(true);
    setError("");
    try {
      if (supabase) await removeIssue(issue.id);
      else
        setIssues((current) => current.filter((item) => item.id !== issue.id));
      await reload();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Không thể xóa issue",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleEdit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editTarget) return;
    const form = new FormData(event.currentTarget);
    const input = {
      creatorName: String(form.get("creatorName")),
      category: String(form.get("category")),
      content: String(form.get("content")),
    };
    setBusy(true);
    setError("");
    try {
      if (supabase) await updateIssue(editTarget.id, input);
      else
        setIssues((current) =>
          current.map((item) =>
            item.id === editTarget.id ? { ...item, ...input } : item,
          ),
        );
      setActiveModal(null);
      setEditTarget(null);
      await reload();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Khong the cap nhat issue",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleVersion(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!versionTarget) return;
    const form = new FormData(event.currentTarget);
    const file = form.get("file") as File | null;
    if (!file || file.size === 0) {
      setError("Hay chon file moi de tao version.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const attachment = await uploadAttachment(file);
      const note = String(form.get("note") ?? "").trim();
      if (supabase) await addIssueVersion(versionTarget, [attachment], note);
      else
        setIssues((current) =>
          current.map((item) =>
            item.id === versionTarget.id
              ? { ...item, attachments: [attachment] }
              : item,
          ),
        );
      setActiveModal(null);
      setVersionTarget(null);
      await reload();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Khong the cap nhat version",
      );
    } finally {
      setBusy(false);
    }
  }

  async function openVersions(issue: Issue) {
    setVersionTarget(issue);
    setMenuTarget(null);
    setBusy(true);
    setError("");
    try {
      setVersions(supabase ? await fetchIssueVersions(issue.id) : []);
      setActiveModal("version");
    } catch (reason) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Khong the tai danh sach version",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleDownload(attachment: Attachment) {
    try {
      await downloadAttachment(attachment);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Khong the tai file");
    }
  }

  async function handleLogout() {
    setBusy(true);
    setError("");
    setProfile(null);
    try { await signOut(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Khong the dang xuat"); }
    finally { setBusy(false); }
  }

  async function openPermissions() {
    if (!canDelete) return;
    setBusy(true); setError("");
    try { setUsers(await fetchProfiles()); setActiveModal("permissions"); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Khong the tai danh sach nguoi dung"); }
    finally { setBusy(false); }
  }

  async function openMembers() {
    setActiveSection("members");
    if (!supabase || users.length) return;
    setBusy(true); setError("");
    try { setUsers(await fetchProfiles()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Khong the tai danh sach thanh vien"); }
    finally { setBusy(false); }
  }

  async function changeRole(id: string, nextRole: UserRole) {
    setBusy(true); setError("");
    try { await updateUserRole(id, nextRole); setUsers((current) => current.map((user) => user.id === id ? { ...user, role: nextRole } : user)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Khong the cap nhat quyen"); }
    finally { setBusy(false); }
  }

  async function changePermission(id: string, permission: "canView" | "canEdit" | "canDelete" | "canDownload", value: boolean) {
    const column = { canView: "can_view", canEdit: "can_edit", canDelete: "can_delete", canDownload: "can_download" }[permission] as "can_view" | "can_edit" | "can_delete" | "can_download";
    setBusy(true); setError("");
    try { await updateUserPermission(id, column, value); setUsers((current) => current.map((user) => user.id === id ? { ...user, [permission]: value } : user)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Khong the cap nhat quyen"); }
    finally { setBusy(false); }
  }

  if (isSupabaseConfigured && !authReady)
    return (
      <div className="auth-loading">
        <Loader2 className="spin" size={24} /> Đang kiểm tra phiên đăng nhập...
      </div>
    );
  if (isSupabaseConfigured && !profile) return <AuthScreen />;
  if (isSupabaseConfigured && profile && !canView) return <div className="auth-screen"><div className="auth-card"><h1>Chưa được cấp quyền xem</h1><p className="auth-note">Quản trị viên cần bật quyền Xem cho tài khoản này.</p><button className="primary-button" onClick={() => void handleLogout()}><LogOut size={17} /> Đăng xuất</button></div></div>;

  return (
    <div className="app-shell">
      <aside className={`sidebar ${mobileNav ? "sidebar-open" : ""}`}>
        <div className="brand">
          <div className="brand-mark">
            <ClipboardList size={22} />
          </div>
          <div>
            <strong>
              VCI<span>.SBGB.ISSUE</span>
            </strong>
              <small>PROJECT ISSUE CONTROL</small>
          </div>
        </div>
        <nav className="main-nav">
          <p className="nav-label">TỔNG QUAN</p>
          <button className={`nav-item ${activeSection === "dashboard" ? "active" : ""}`} onClick={() => setActiveSection("dashboard")}>
            <LayoutDashboard size={18} /> Bảng theo dõi
          </button>
          <button className={`nav-item ${activeSection === "members" ? "active" : ""}`} onClick={() => void openMembers()}>
            <UsersRound size={18} /> Thành viên
          </button>
          <button className={`nav-item ${activeSection === "library" ? "active" : ""}`} onClick={() => setActiveSection("library")}>
            <FolderOpen size={18} /> Thư viện hồ sơ
          </button>
          <p className="nav-label nav-gap">HỆ THỐNG</p>
          <button className="nav-item" disabled={!canDelete} onClick={() => void openPermissions()}>
            <ShieldCheck size={18} /> Phân quyền
          </button>
          <button className="nav-item">
            <Settings size={18} /> Cài đặt
          </button>
        </nav>
        <div className="sidebar-footer">
          <div className="support-card">
            <CircleHelp size={19} />
            <div>
              <strong>Cần hỗ trợ?</strong>
              <span>Liên hệ quản trị viên</span>
            </div>
          </div>
          <button
            className="nav-item logout"
            onClick={() => void handleLogout()}
            disabled={busy}
          >
            <LogOut size={18} /> Đăng xuất
          </button>
        </div>
      </aside>
      {mobileNav && (
        <button
          className="mobile-overlay"
          onClick={() => setMobileNav(false)}
          aria-label="Đóng menu"
        />
      )}
      <main className="main-content">
        <header className="topbar">
          <button
            className="mobile-menu icon-button"
            onClick={() => setMobileNav(true)}
          >
            <Menu size={21} />
          </button>
          <div className="breadcrumb">
            <span>DỰ ÁN</span>
            <ChevronRight size={14} />
            <strong>VCI.SBGB.ISSUE</strong>
          </div>
          <div className="top-actions">
            <div className="role-switch">
              <UserRound size={15} />
              {supabase ? (
                <>
                  <span>{profile?.fullName || profile?.email}</span>
                  <span className="role-pill">{roleLabel[role]}</span>
                </>
              ) : (
                <>
                  <select
                    value={role}
                    onChange={(event) =>
                      setRole(event.target.value as UserRole)
                    }
                    aria-label="Vai trò demo"
                  >
                    <option value="admin">Admin</option>
                    <option value="editor">Editor</option>
                    <option value="viewer">Viewer</option>
                  </select>
                  <ChevronDown size={13} />
                </>
              )}
            </div>
            <button className="notification icon-button">
              <Bell size={19} />
              <i />
            </button>
            <div className="avatar">
              {profile?.fullName?.slice(0, 2).toUpperCase() || "NV"}
            </div>
          </div>
        </header>
        {activeSection === "members" ? <MembersPage users={users} busy={busy} /> : activeSection === "library" ? <LibraryPage issues={issues} canDownload={canDownload} onDownload={handleDownload} /> : <>
        <section className="page-heading">
          <div>
            <p className="eyebrow blue">QUẢN LÝ PHỐI HỢP DỰ ÁN</p>
            <h1>
              Bảng theo dõi ý kiến <span>(Issue)</span>
            </h1>
            <p className="heading-note">
              Theo dõi, phản hồi và lưu trữ toàn bộ ý kiến trong một không gian
              thống nhất.
            </p>
          </div>
          {canCreate && (
            <button
              className="primary-button"
              onClick={() => setActiveModal("create")}
            >
              <Plus size={18} /> Tạo Issue mới
            </button>
          )}
        </section>
        {error && (
          <div className="alert-error">
            <X size={16} /> {error}
          </div>
        )}
        <section className="stats-grid">
          <div className="stat-card">
            <div className="stat-icon stat-blue">
              <ClipboardList size={19} />
            </div>
            <div>
              <span>Tổng số Issue</span>
              <strong>{stats.total}</strong>
            </div>
            <small className="trend">
              +12% <em>so với tháng trước</em>
            </small>
          </div>
          <div className="stat-card">
            <div className="stat-icon stat-orange">
              <MessageSquareText size={19} />
            </div>
            <div>
              <span>Đang xử lý</span>
              <strong>{stats.open}</strong>
            </div>
            <small className="trend orange">Cần phản hồi</small>
          </div>
          <div className="stat-card">
            <div className="stat-icon stat-green">
              <Check size={19} />
            </div>
            <div>
              <span>Đã trả lời</span>
              <strong>{stats.answered}</strong>
            </div>
            <small className="trend green">Đã hoàn tất</small>
          </div>
        </section>
        <section className="table-card">
          <div className="table-toolbar">
            <div>
              <h2>Danh sách Issue</h2>
              <p>
                {loading
                  ? "Đang đồng bộ dữ liệu..."
                  : `${filteredIssues.length} issue trong hệ thống`}
              </p>
            </div>
            <div className="toolbar-actions">
              <label className="search-box">
                <Search size={17} />
                <input
                  value={query}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setPage(1);
                  }}
                  placeholder="Tìm theo nội dung, người tạo..."
                />
              </label>
              <div className="filter-select">
                <Filter size={15} />
                <select
                  value={category}
                  onChange={(event) => {
                    setCategory(event.target.value);
                    setPage(1);
                  }}
                >
                  <option>Tất cả hạng mục</option>
                  {categoryOptions.map((item) => (
                    <option key={item}>{item}</option>
                  ))}
                </select>
                <ChevronDown size={14} />
              </div>
              <div className="filter-select">
                <select
                  value={status}
                  onChange={(event) => {
                    setStatus(event.target.value);
                    setPage(1);
                  }}
                >
                  <option>Tất cả trạng thái</option>
                  {STATUS_OPTIONS.map((item) => (
                    <option key={item}>{item}</option>
                  ))}
                </select>
                <ChevronDown size={14} />
              </div>
            </div>
          </div>
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th className="index-col">
                    STT <ChevronDown size={12} />
                  </th>
                  <th>Người tạo</th>
                  <th>Hạng mục</th>
                  <th className="content-col">Nội dung</th>
                  <th>Đính kèm</th>
                  <th>Ngày tạo</th>
                  <th>Trả lời</th>
                  <th>Người trả lời</th>
                  <th>Ngày trả lời</th>
                  <th>Trạng thái</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr>
                    <td colSpan={11} className="empty-state">
                      <Loader2 className="spin" /> Đang tải dữ liệu...
                    </td>
                  </tr>
                ) : visibleIssues.length === 0 ? (
                  <tr>
                    <td colSpan={11} className="empty-state">
                      Không tìm thấy issue phù hợp.
                    </td>
                  </tr>
                ) : (
                  visibleIssues.map((issue, index) => (
                    <tr key={issue.id}>
                      <td className="index-col">
                        {(page - 1) * pageSize + index + 1}
                      </td>
                      <td>
                        <div className="creator-cell">
                          <div className="mini-avatar">
                            {issue.creatorName
                              .split(" ")
                              .map((part) => part[0])
                              .slice(-2)
                              .join("")}
                          </div>
                          <span>{issue.creatorName}</span>
                        </div>
                      </td>
                      <td>
                        <span className="category-text">{issue.category}</span>
                      </td>
                      <td className="content-cell">
                        <span>
                          {issue.content.length > 92
                            ? `${issue.content.slice(0, 92)}...`
                            : issue.content}
                        </span>
                        {issue.content.length > 92 && (
                          <button
                            className="more-link"
                            onClick={() => {
                              setViewTarget(issue);
                              setActiveModal("view");
                            }}
                          >
                            Xem thêm
                          </button>
                        )}
                      </td>
                      <td>
                        <div className="attachments">
                          {issue.attachments.length ? (
                            issue.attachments.map((attachment) => (
                              <button
                                key={`${issue.id}-${attachment.name}`}
                                className={attachmentClass(attachment)}
                                disabled={!canDownload}
                                type="button"
                                onClick={() => void handleDownload(attachment)}
                                title={attachment.name}
                              >
                                {fileIcon(attachment)}
                                <span>
                                  {attachment.name.length > 13
                                    ? `${attachment.name.slice(0, 10)}...`
                                    : attachment.name}
                                </span>
                                <Download size={11} />
                              </button>
                            ))
                          ) : (
                            <span className="no-file">-</span>
                          )}
                        </div>
                      </td>
                      <td className="date-cell">
                        {formatDate(issue.createdAt)}
                      </td>
                      <td className="reply-cell">
                        {issue.reply ? (
                          issue.reply.length > 55 ? (
                            `${issue.reply.slice(0, 55)}...`
                          ) : (
                            issue.reply
                          )
                        ) : (
                          <span className="muted">Chưa có phản hồi</span>
                        )}
                      </td>
                      <td>
                        {issue.responderName || (
                          <span className="muted">-</span>
                        )}
                      </td>
                      <td className="date-cell">
                        {formatDate(issue.repliedAt)}
                      </td>
                      <td>
                        <StatusBadge status={issue.status} />
                      </td>
                      <td>
                        <div className="row-actions">
                          {canEdit && (
                            <button
                              className="row-icon"
                              title="Trả lời"
                              onClick={() => {
                                setAnswerTarget(issue);
                                setActiveModal("answer");
                              }}
                            >
                              <MessageSquareText size={16} />
                            </button>
                          )}
                          {canEdit && (
                            <button className="row-icon" title="Chỉnh sửa" onClick={() => { setEditTarget(issue); setMenuTarget(null); setActiveModal("edit"); }}>
                              <Pencil size={16} />
                            </button>
                          )}
                          {canDelete && (
                            <button
                              className="row-icon danger"
                              title="Xóa"
                              onClick={() => handleDelete(issue)}
                            >
                              <Trash2 size={16} />
                            </button>
                          )}
                          <div className="more-menu-wrap">
                            <button className="row-icon" title="Thêm thao tác" onClick={() => setMenuTarget(menuTarget === issue.id ? null : issue.id)}><MoreHorizontal size={17} /></button>
                            {menuTarget === issue.id && <div className="more-menu">
                              <button onClick={() => { setVersionTarget(issue); setMenuTarget(null); setVersions([]); setActiveModal("version"); }}><Upload size={14} /> Cập nhật version</button>
                              <button onClick={() => void openVersions(issue)}><Archive size={14} /> Xem danh sách version</button>
                            </div>}
                          </div>
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
          <div className="table-footer">
            <span>
              Hiển thị {visibleIssues.length ? (page - 1) * pageSize + 1 : 0}-
              {Math.min(page * pageSize, filteredIssues.length)} trên{" "}
              {filteredIssues.length}
            </span>
            <div className="pagination">
              <label>
                Hiển thị{" "}
                <select
                  value={pageSize}
                  onChange={(event) => {
                    setPageSize(Number(event.target.value));
                    setPage(1);
                  }}
                >
                  <option value={10}>10</option>
                  <option value={20}>20</option>
                </select>{" "}
                / trang
              </label>
              <button
                className="page-button"
                disabled={page <= 1}
                onClick={() => setPage((current) => current - 1)}
              >
                <ChevronLeft size={16} />
              </button>
              <span className="page-current">{page}</span>
              <button
                className="page-button"
                disabled={page >= totalPages}
                onClick={() => setPage((current) => current + 1)}
              >
                <ChevronRight size={16} />
              </button>
            </div>
          </div>
        </section>
        </>}
        <footer className="page-footer">
          <span>
            <CloudUpload size={14} /> Dữ liệu được đồng bộ an toàn
          </span>
          <span>
            {isSupabaseConfigured ? "Supabase Cloud" : "Demo Mock Data"} · Cập
            nhật lúc{" "}
            {new Date().toLocaleTimeString("vi-VN", {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        </footer>
      </main>
      {activeModal === "create" && (
        <CreateModal
          busy={busy}
          onClose={() => setActiveModal(null)}
          onSubmit={handleCreate}
        />
      )}
      {activeModal === "answer" && answerTarget && (
        <AnswerModal
          issue={answerTarget}
          busy={busy}
          onClose={() => {
            setActiveModal(null);
            setAnswerTarget(null);
          }}
          onSubmit={handleAnswer}
        />
      )}
      {activeModal === "edit" && editTarget && (
        <EditModal issue={editTarget} busy={busy} onClose={() => { setActiveModal(null); setEditTarget(null); }} onSubmit={handleEdit} />
      )}
      {activeModal === "version" && versionTarget && (
        <VersionModal issue={versionTarget} versions={versions} busy={busy} onClose={() => { setActiveModal(null); setVersionTarget(null); }} onSubmit={handleVersion} onDownload={handleDownload} />
      )}
      {activeModal === "permissions" && <PermissionsDetailModal users={users} busy={busy} onClose={() => setActiveModal(null)} onChangeRole={changeRole} onChangePermission={changePermission} />}
      {activeModal === "view" && viewTarget && (
        <Modal
          title="Chi tiết nội dung Issue"
          onClose={() => {
            setActiveModal(null);
            setViewTarget(null);
          }}
          wide
        >
          <div className="detail-meta">
            <StatusBadge status={viewTarget.status} />
            <span>{formatDate(viewTarget.createdAt)}</span>
            <span>{viewTarget.category}</span>
          </div>
          <div className="detail-content">{viewTarget.content}</div>
          {viewTarget.attachments.length > 0 && (
            <div className="detail-files">
              <h3>Hồ sơ đính kèm</h3>
              {viewTarget.attachments.map((attachment) => (
                <button
                  key={attachment.name}
                  type="button"
                  onClick={() => void handleDownload(attachment)}
                  className="detail-file"
                  disabled={!canDownload}
                >
                  {fileIcon(attachment)} {attachment.name}
                  <ArrowDownToLine size={15} />
                </button>
              ))}
            </div>
          )}
        </Modal>
      )}
    </div>
  );
}

function AuthScreen() {
  const [mode, setMode] = useState<"login" | "signup">("login");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const params = new URLSearchParams(window.location.hash.replace(/^#/, ""));
    if (params.get("error_code") === "otp_expired") {
      setError("Link xác nhận email đã hết hạn hoặc đã được sử dụng. Hãy đăng ký lại hoặc yêu cầu gửi lại email mới.");
      window.history.replaceState(null, "", window.location.pathname);
    }
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const email = String(form.get("email"));
      const password = String(form.get("password"));
      if (mode === "signup") {
        await signUp(email, password, String(form.get("fullName")));
        setMessage(
          "Tài khoản đã tạo. Hãy kiểm tra email nếu Supabase yêu cầu xác nhận.",
        );
      } else {
        await signIn(email, password);
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Đăng nhập thất bại");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth-screen">
      <div className="auth-card">
        <div className="auth-brand">
          <div className="brand-mark">
            <ClipboardList size={22} />
          </div>
          <div>
            <strong>
              VCI<span>.SBGB.ISSUE</span>
            </strong>
              <small>PROJECT ISSUE CONTROL</small>
          </div>
        </div>
        <p className="eyebrow blue">KHÔNG GIAN LÀM VIỆC</p>
        <h1>{mode === "login" ? "Chào mừng trở lại" : "Tạo tài khoản"}</h1>
        <p className="auth-note">
          Đăng nhập để quản lý issue, hồ sơ và quyền truy cập dự án.
        </p>
        {error && (
          <div className="alert-error auth-alert">
            <X size={16} /> {error}
          </div>
        )}
        {message && (
          <div className="auth-success">
            <Check size={16} /> {message}
          </div>
        )}
        <form className="issue-form auth-form" onSubmit={submit}>
          {mode === "signup" && (
            <label>
              Họ và tên <span>*</span>
              <input name="fullName" required placeholder="Nguyễn Văn A" />
            </label>
          )}
          <label>
            Email <span>*</span>
            <input
              name="email"
              type="email"
              required
              placeholder="you@company.com"
            />
          </label>
          <label>
            Mật khẩu <span>*</span>
            <input
              name="password"
              type="password"
              minLength={6}
              required
              placeholder="Tối thiểu 6 ký tự"
            />
          </label>
          <button className="primary-button" disabled={busy} type="submit">
            {busy ? (
              <Loader2 className="spin" size={17} />
            ) : (
              <ShieldCheck size={17} />
            )}{" "}
            {mode === "login" ? "Đăng nhập" : "Đăng ký"}
          </button>
        </form>
        <button
          className="auth-toggle"
          onClick={() => {
            setMode(mode === "login" ? "signup" : "login");
            setError("");
            setMessage("");
          }}
        >
          {mode === "login"
            ? "Chưa có tài khoản? Đăng ký"
            : "Đã có tài khoản? Đăng nhập"}
        </button>
      </div>
    </div>
  );
}

function PermissionsModal({ users, busy, onClose, onChangeRole }: { users: UserProfile[]; busy: boolean; onClose: () => void; onChangeRole: (id: string, role: UserRole) => void }) {
  return <Modal title="Phân quyền người dùng" onClose={onClose} wide><div className="permissions-list">{users.length === 0 ? <p className="muted">Chưa có người dùng nào.</p> : users.map((user) => <div className="permission-row" key={user.id}><div><strong>{user.fullName || "Chưa đặt tên"}</strong><small>{user.id}</small></div><select value={user.role} disabled={busy} onChange={(event) => void onChangeRole(user.id, event.target.value as UserRole)}><option value="admin">Admin</option><option value="editor">Editor</option><option value="viewer">Viewer</option></select></div>)}</div></Modal>;
}

function PermissionsDetailModal({ users, busy, onClose, onChangeRole, onChangePermission }: { users: UserProfile[]; busy: boolean; onClose: () => void; onChangeRole: (id: string, role: UserRole) => void; onChangePermission: (id: string, permission: "canView" | "canEdit" | "canDelete" | "canDownload", value: boolean) => void }) {
  const permissions = [["canView", "Xem"], ["canEdit", "Sua"], ["canDelete", "Xoa"], ["canDownload", "Tai file"]] as const;
  return <Modal title="Permissions" onClose={onClose} wide><div className="permissions-list">{users.map((user) => <div className="permission-row" key={user.id}><div><strong>{user.fullName || "Chua dat ten"}</strong><small>{user.id}</small><div className="permission-checks">{permissions.map(([key, label]) => <label key={key}><input type="checkbox" checked={user[key]} disabled={busy} onChange={(event) => void onChangePermission(user.id, key, event.target.checked)} /> {label}</label>)}</div></div><select value={user.role} disabled={busy} onChange={(event) => void onChangeRole(user.id, event.target.value as UserRole)}><option value="admin">Admin</option><option value="editor">Editor</option><option value="viewer">Viewer</option></select></div>)}</div></Modal>;
}

function MembersPage({ users, busy }: { users: UserProfile[]; busy: boolean }) {
  return <section className="feature-page"><div className="feature-heading"><div><p className="eyebrow blue">WORKSPACE</p><h1>Thành viên</h1><p className="heading-note">Danh sách người dùng và vai trò trong dự án.</p></div><UsersRound size={38} /></div><div className="feature-card"><div className="feature-card-title"><h2>Danh sách thành viên</h2><span>{users.length} tài khoản</span></div>{busy ? <p className="muted">Đang tải...</p> : users.length === 0 ? <p className="muted">Chưa có dữ liệu thành viên.</p> : users.map((user) => <div className="member-row" key={user.id}><div className="mini-avatar">{user.fullName.slice(0, 2).toUpperCase()}</div><div><strong>{user.fullName || "Chưa đặt tên"}</strong><small>ID: {user.id}</small></div><span className="role-pill">{roleLabel[user.role]}</span></div>)}</div></section>;
}

function LibraryPage({ issues, canDownload, onDownload }: { issues: Issue[]; canDownload: boolean; onDownload: (attachment: Attachment) => void }) {
  const [query, setQuery] = useState("");
  const files = issues.flatMap((issue) => issue.attachments.map((attachment) => ({ issue, attachment })));
  const filtered = files.filter(({ issue, attachment }) => `${attachment.name} ${issue.category} ${issue.creatorName}`.toLowerCase().includes(query.toLowerCase()));
  return <section className="feature-page"><div className="feature-heading"><div><p className="eyebrow blue">DOCUMENT CONTROL</p><h1>Thư viện hồ sơ</h1><p className="heading-note">Tập trung toàn bộ file đính kèm và hồ sơ theo Issue.</p></div><FolderOpen size={38} /></div><div className="feature-card"><div className="feature-card-title"><h2>Hồ sơ đính kèm</h2><label className="search-box library-search"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Tìm tên file..." /></label></div>{filtered.length === 0 ? <p className="muted">Chưa có hồ sơ phù hợp.</p> : filtered.map(({ issue, attachment }) => <div className="library-row" key={`${issue.id}-${attachment.name}`}><span className={attachmentClass(attachment)}>{fileIcon(attachment)}</span><div><strong>{attachment.name}</strong><small>{issue.category} · {issue.creatorName}</small></div><button className="secondary-button" disabled={!canDownload} onClick={() => onDownload(attachment)}><Download size={15} /> Tải file</button></div>)}</div></section>;
}

function CreateModal({
  onClose,
  onSubmit,
  busy,
}: {
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  busy: boolean;
}) {
  const [fileName, setFileName] = useState("");
  return (
    <Modal title="Tạo Issue mới" onClose={onClose}>
      <form className="issue-form" onSubmit={onSubmit}>
        <div className="form-grid">
          <label>
            Người tạo <span>*</span>
            <input name="creatorName" required placeholder="Nhập họ và tên" />
          </label>
          <label>
            Hạng mục <span>*</span>
            <select name="category" required defaultValue="">
              <option value="" disabled>
                Chọn hạng mục
              </option>
              {categoryOptions.map((item) => (
                <option key={item}>{item}</option>
              ))}
            </select>
          </label>
        </div>
        <label>
          Nội dung ý kiến <span>*</span>
          <textarea
            name="content"
            required
            rows={6}
            placeholder="Mô tả chi tiết ý kiến hoặc sự cố..."
          />
          <small>Thời gian tạo sẽ được ghi nhận tự động khi gửi.</small>
        </label>
        <div className="form-grid">
          <label className="file-drop">
            <span>Đính kèm tệp</span>
            <input
              name="file"
              type="file"
              accept=".pdf,.doc,.docx,.xls,.xlsx,.png,.jpg,.jpeg,.zip,.dwg"
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setFileName(event.target.files?.[0]?.name ?? "")
              }
            />
            <span className="file-drop-box">
              <Upload size={18} />
              {fileName || "Kéo thả hoặc bấm để chọn file"}
            </span>
            <small>PDF, DOCX, XLSX, PNG, JPG, ZIP, DWG...</small>
          </label>
          <label>
            Hoặc dán đường link
            <input name="link" type="url" placeholder="https://..." />
            <small>Link hồ sơ trên Drive, SharePoint...</small>
          </label>
        </div>
        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            Hủy
          </button>
          <button className="primary-button" disabled={busy} type="submit">
            {busy ? <Loader2 className="spin" size={17} /> : <Plus size={17} />}{" "}
            Tạo Issue
          </button>
        </div>
      </form>
    </Modal>
  );
}

function EditModal({ issue, onClose, onSubmit, busy }: { issue: Issue; onClose: () => void; onSubmit: (event: FormEvent<HTMLFormElement>) => void; busy: boolean }) {
  return <Modal title="Chỉnh sửa Issue" onClose={onClose}><form className="issue-form" onSubmit={onSubmit}>
    <label>Người tạo <span>*</span><input name="creatorName" required defaultValue={issue.creatorName} /></label>
    <label>Hạng mục <span>*</span><select name="category" required defaultValue={issue.category}>{categoryOptions.map((item) => <option key={item}>{item}</option>)}</select></label>
    <label>Nội dung ý kiến <span>*</span><textarea name="content" required rows={7} defaultValue={issue.content} /></label>
    <div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>Hủy</button><button className="primary-button" disabled={busy} type="submit">{busy ? <Loader2 className="spin" size={17} /> : <Pencil size={17} />} Lưu thay đổi</button></div>
  </form></Modal>;
}

function VersionModal({ issue, versions, onClose, onSubmit, onDownload, busy }: { issue: Issue; versions: IssueVersion[]; onClose: () => void; onSubmit: (event: FormEvent<HTMLFormElement>) => void; onDownload: (attachment: Attachment) => void; busy: boolean }) {
  return <Modal title={`Hồ sơ version - ${issue.category}`} onClose={onClose} wide>
    <form className="issue-form" onSubmit={onSubmit}><label>Cập nhật file mới <span>*</span><input name="file" type="file" required accept=".pdf,.doc,.docx,.xls,.xlsx,.png,.jpg,.jpeg,.zip,.dwg" /><small>File mới sẽ trở thành bản hiện hành, bản cũ vẫn được giữ lại.</small></label><label>Ghi chú version<textarea name="note" rows={2} placeholder="Ví dụ: Đã chỉnh cao độ tầng 3" /></label><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>Đóng</button><button className="primary-button" disabled={busy} type="submit">{busy ? <Loader2 className="spin" size={17} /> : <Upload size={17} />} Tạo version mới</button></div></form>
    <div className="version-list"><h3>Lịch sử version</h3>{versions.length === 0 ? <p className="muted">Chưa có version cũ trong hệ thống.</p> : versions.map((version) => <div className="version-item" key={version.id}><div><strong>Version {version.versionNumber}</strong><small>{formatDate(version.createdAt)} {version.note && ` · ${version.note}`}</small></div>{version.attachments.map((attachment) => <button type="button" className="detail-file version-file" key={attachment.name} onClick={() => onDownload(attachment)}>{fileIcon(attachment)} {attachment.name}<Download size={14} /></button>)}</div>)}</div>
  </Modal>;
}

function AnswerModal({
  issue,
  onClose,
  onSubmit,
  busy,
}: {
  issue: Issue;
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  busy: boolean;
}) {
  return (
    <Modal title="Trả lời Issue" onClose={onClose}>
      <div className="quoted-issue">
        <span>{issue.category}</span>
        <p>{issue.content}</p>
        <small>
          Người tạo: {issue.creatorName} · {formatDate(issue.createdAt)}
        </small>
      </div>
      <form className="issue-form" onSubmit={onSubmit}>
        <label>
          Người trả lời <span>*</span>
          <input
            name="responderName"
            required
            defaultValue=""
            placeholder="Nhập họ và tên người trả lời"
          />
        </label>
        <label>
          Nội dung phản hồi <span>*</span>
          <textarea
            name="reply"
            required
            rows={5}
            defaultValue={issue.reply}
            placeholder="Nhập nội dung trả lời..."
          />
          <small>Thời gian trả lời sẽ được ghi nhận tự động khi gửi.</small>
        </label>
        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onClose}>
            Hủy
          </button>
          <button className="primary-button" disabled={busy} type="submit">
            {busy ? (
              <Loader2 className="spin" size={17} />
            ) : (
              <Check size={17} />
            )}{" "}
            Gửi phản hồi
          </button>
        </div>
      </form>
    </Modal>
  );
}

export default App;
