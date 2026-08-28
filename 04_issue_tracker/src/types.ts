export type IssueStatus = "Mới tạo" | "Đang xử lý" | "Đã trả lời";
export type UserRole = "admin" | "editor" | "viewer";

export type Attachment = {
  id?: string;
  name: string;
  url: string;
  kind: "file" | "link";
  mimeType?: string;
};

export type Issue = {
  id: string;
  creatorName: string;
  category: string;
  content: string;
  attachments: Attachment[];
  createdAt: string;
  reply: string;
  responderName: string;
  repliedAt: string | null;
  status: IssueStatus;
};

export const STATUS_OPTIONS: IssueStatus[] = ["Mới tạo", "Đang xử lý", "Đã trả lời"];
