export interface Env {
  FILES: R2Bucket;
  SUPABASE_URL: string;
  SUPABASE_ANON_KEY: string;
  SUPABASE_SERVICE_ROLE_KEY: string;
}

const cors = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "authorization, content-type", "Access-Control-Allow-Methods": "GET, POST, OPTIONS" };

function response(body: BodyInit | null, status = 200, extra: HeadersInit = {}) {
  return new Response(body, { status, headers: { ...cors, "Content-Type": "application/json", ...extra } });
}

async function currentUser(request: Request, env: Env) {
  const token = request.headers.get("Authorization");
  if (!token) return null;
  const userResponse = await fetch(`${env.SUPABASE_URL}/auth/v1/user`, { headers: { apikey: env.SUPABASE_ANON_KEY, Authorization: token } });
  if (!userResponse.ok) return null;
  const user = await userResponse.json() as { id: string };
  const profileResponse = await fetch(`${env.SUPABASE_URL}/rest/v1/profiles?id=eq.${user.id}&select=role,can_edit,can_download`, { headers: { apikey: env.SUPABASE_SERVICE_ROLE_KEY, Authorization: `Bearer ${env.SUPABASE_SERVICE_ROLE_KEY}` } });
  const profiles = await profileResponse.json() as Array<{ role: string; can_edit: boolean; can_download: boolean }>;
  return profiles[0] ? { ...user, ...profiles[0] } : null;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method === "OPTIONS") return new Response(null, { headers: cors });
    const url = new URL(request.url);
    const user = await currentUser(request, env);
    if (!user) return response(JSON.stringify({ error: "Unauthorized" }), 401);

    if (request.method === "POST" && url.pathname === "/upload") {
      if (!user.can_edit) return response(JSON.stringify({ error: "Bạn không có quyền sửa/upload file" }), 403);
      const form = await request.formData();
      const file = form.get("file");
      if (!(file instanceof File)) return response(JSON.stringify({ error: "Missing file" }), 400);
      if (file.size > 50 * 1024 * 1024) return response(JSON.stringify({ error: "File tối đa 50 MB" }), 413);
      const safeName = file.name.replace(/[^a-zA-Z0-9._-]/g, "-");
      const key = `issues/${crypto.randomUUID()}-${safeName}`;
      await env.FILES.put(key, file.stream(), { httpMetadata: { contentType: file.type || "application/octet-stream" } });
      return response(JSON.stringify({ name: file.name, url: `${url.origin}/download?key=${encodeURIComponent(key)}`, kind: "file", mimeType: file.type }));
    }

    if (request.method === "GET" && url.pathname === "/download") {
      if (!user.can_download) return response(JSON.stringify({ error: "Bạn không có quyền tải file" }), 403);
      const key = url.searchParams.get("key");
      if (!key) return response(JSON.stringify({ error: "Missing key" }), 400);
      const object = await env.FILES.get(key);
      if (!object) return response(JSON.stringify({ error: "File not found" }), 404);
      const headers = new Headers(cors);
      object.writeHttpMetadata(headers);
      headers.set("etag", object.httpEtag);
      headers.set("Content-Disposition", `attachment; filename="${key.split("/").pop()?.replace(/^[^-]+-/, "") || "download"}"`);
      return new Response(object.body, { headers });
    }
    return response(JSON.stringify({ error: "Not found" }), 404);
  },
};
