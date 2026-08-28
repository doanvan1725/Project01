import { supabase } from "./supabase";
import type { Attachment, Issue, IssueStatus } from "../types";
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
  const { data, error } = await supabase.from("issues").select("*").order("created_at", { ascending: false });
  if (error) throw error;
  return (data as IssueRow[]).map(fromRow);
}

export async function getCurrentProfile(): Promise<{ email: string; fullName: string; role: UserRole } | null> {
  if (!supabase) return null;
  const { data: userData } = await supabase.auth.getUser();
  if (!userData.user?.email) return null;
  const { data, error } = await supabase.from("profiles").select("full_name, role").eq("id", userData.user.id).single();
  if (error) throw error;
  return { email: userData.user.email, fullName: data.full_name, role: data.role as UserRole };
}

export async function signIn(email: string, password: string): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw error;
}

export async function signUp(email: string, password: string, fullName: string): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signUp({ email, password, options: { data: { full_name: fullName } } });
  if (error) throw error;
}

export async function createIssue(input: Pick<Issue, "creatorName" | "category" | "content" | "attachments">): Promise<Issue> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { data, error } = await supabase.from("issues").insert({
    creator_name: input.creatorName,
    category: input.category,
    content: input.content,
    attachments: input.attachments,
    status: "Mới tạo",
  }).select().single();
  if (error) throw error;
  return fromRow(data as IssueRow);
}

export async function answerIssue(id: string, responderName: string, reply: string): Promise<void> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { error } = await supabase.from("issues").update({
    responder_name: responderName,
    reply,
    replied_at: new Date().toISOString(),
    status: "Đã trả lời",
  }).eq("id", id);
  if (error) throw error;
}

export async function removeIssue(id: string): Promise<void> {
  if (!supabase) throw new Error("Supabase chưa được cấu hình");
  const { error } = await supabase.from("issues").delete().eq("id", id);
  if (error) throw error;
}

export async function uploadAttachment(file: File): Promise<Attachment> {
  if (!supabase) return { name: file.name, url: URL.createObjectURL(file), kind: "file", mimeType: file.type };
  const safeName = `${crypto.randomUUID()}-${file.name.replace(/[^a-zA-Z0-9._-]/g, "-")}`;
  const { error } = await supabase.storage.from("issue-files").upload(safeName, file);
  if (error) throw error;
  const { data } = supabase.storage.from("issue-files").getPublicUrl(safeName);
  return { name: file.name, url: data.publicUrl, kind: "file", mimeType: file.type };
}
