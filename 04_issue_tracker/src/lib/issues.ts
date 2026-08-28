import { supabase } from "./supabase";
import type { Attachment, Issue, IssueStatus, IssueVersion } from "../types";
import type { UserRole } from "../types";

type IssueRow = {
  id: string;
  creator_name: string;
  category: string;
  content: string;
  attachments: Attachment[] | null;
  created_at: string;
  reply: string | null;
  responder_name: string | null;
  replied_at: string | null;
  status: IssueStatus;
};

type VersionRow = {
  id: string;
  issue_id: string;
  version_number: number;
  attachments: Attachment[] | null;
  note: string | null;
  created_at: string;
};

const fromRow = (row: IssueRow): Issue => ({
  id: row.id,
  creatorName: row.creator_name,
  category: row.category,
  content: row.content,
  attachments: row.attachments ?? [],
  createdAt: row.created_at,
  reply: row.reply ?? "",
  responderName: row.responder_name ?? "",
  repliedAt: row.replied_at,
  status: row.status,
});

export async function fetchIssues(): Promise<Issue[]> {
  if (!supabase) return [];
  const { data, error } = await supabase
    .from("issues")
    .select("*")
    .order("created_at", { ascending: false });
  if (error) throw error;
  return (data as IssueRow[]).map(fromRow);
}

export async function getCurrentProfile(): Promise<{
  email: string;
  fullName: string;
  role: UserRole;
} | null> {
  if (!supabase) return null;
  const { data: userData } = await supabase.auth.getUser();
  if (!userData.user?.email) return null;
  const { data, error } = await supabase
    .from("profiles")
    .select("full_name, role")
    .eq("id", userData.user.id)
    .maybeSingle();
  if (error) throw error;
  if (!data) throw new Error("Tai khoan chua co ho so quyen. Hay chay schema.sql trong Supabase SQL Editor.");
  return {
    email: userData.user.email,
    fullName: data.full_name,
    role: data.role as UserRole,
  };
}

export async function signIn(email: string, password: string): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw error;
}

export async function signOut(): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signOut();
  if (error) throw error;
}

export async function signUp(
  email: string,
  password: string,
  fullName: string,
): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signUp({
    email,
    password,
    options: { data: { full_name: fullName } },
  });
  if (error) throw error;
}

export async function createIssue(
  input: Pick<Issue, "creatorName" | "category" | "content" | "attachments">,
): Promise<Issue> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { data, error } = await supabase
    .from("issues")
    .insert({
      creator_name: input.creatorName,
      category: input.category,
      content: input.content,
      attachments: input.attachments,
      status: "Mới tạo",
    })
    .select()
    .single();
  if (error) throw error;
  return fromRow(data as IssueRow);
}

export async function answerIssue(
  id: string,
  responderName: string,
  reply: string,
): Promise<void> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { error } = await supabase
    .from("issues")
    .update({
      responder_name: responderName,
      reply,
      replied_at: new Date().toISOString(),
      status: "Đã trả lời",
    })
    .eq("id", id);
  if (error) throw error;
}

export async function updateIssue(
  id: string,
  input: Pick<Issue, "creatorName" | "category" | "content">,
): Promise<void> {
  if (!supabase) throw new Error("Supabase chua duoc cau hinh");
  const { error } = await supabase
    .from("issues")
    .update({
      creator_name: input.creatorName,
      category: input.category,
      content: input.content,
    })
    .eq("id", id);
  if (error) throw error;
}

export async function addIssueVersion(
  issue: Issue,
  attachments: Attachment[],
  note: string,
): Promise<void> {
  if (!supabase) throw new Error("Supabase chua duoc cau hinh");
  const { data: latest, error: latestError } = await supabase
    .from("issue_versions")
    .select("version_number")
    .eq("issue_id", issue.id)
    .order("version_number", { ascending: false })
    .limit(1)
    .maybeSingle();
  if (latestError) throw latestError;
  const { error: versionError } = await supabase
    .from("issue_versions")
    .insert({
      issue_id: issue.id,
      version_number: (latest?.version_number ?? 0) + 1,
      attachments,
      note,
    });
  if (versionError) throw versionError;
  const { error: issueError } = await supabase
    .from("issues")
    .update({ attachments })
    .eq("id", issue.id);
  if (issueError) throw issueError;
}

export async function fetchIssueVersions(
  issueId: string,
): Promise<IssueVersion[]> {
  if (!supabase) return [];
  const { data, error } = await supabase
    .from("issue_versions")
    .select("*")
    .eq("issue_id", issueId)
    .order("version_number", { ascending: false });
  if (error) throw error;
  return (data as VersionRow[]).map((row) => ({
    id: row.id,
    issueId: row.issue_id,
    versionNumber: row.version_number,
    attachments: row.attachments ?? [],
    note: row.note ?? "",
    createdAt: row.created_at,
  }));
}

export async function removeIssue(id: string): Promise<void> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { error } = await supabase.from("issues").delete().eq("id", id);
  if (error) throw error;
}

export async function uploadAttachment(file: File): Promise<Attachment> {
  if (!supabase)
    return {
      name: file.name,
      url: URL.createObjectURL(file),
      kind: "file",
      mimeType: file.type,
    };
  const safeName = `issues/${crypto.randomUUID()}-${file.name.replace(/[^a-zA-Z0-9._-]/g, "-")}`;
  const { error } = await supabase.storage
    .from("issue-files")
    .upload(safeName, file);
  if (error) throw error;
  const { data } = supabase.storage.from("issue-files").getPublicUrl(safeName);
  return {
    name: file.name,
    url: data.publicUrl,
    kind: "file",
    mimeType: file.type,
  };
}

export async function downloadAttachment(
  attachment: Attachment,
): Promise<void> {
  if (attachment.kind === "link") {
    window.open(attachment.url, "_blank", "noopener,noreferrer");
    return;
  }
  const response = await fetch(attachment.url);
  if (!response.ok) throw new Error(`Khong the tai file (${response.status})`);
  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = objectUrl;
  anchor.download = attachment.name;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(objectUrl);
}
