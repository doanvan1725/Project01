# Cloudflare R2 Worker

## 1. Create Cloudflare account

1. Open https://dash.cloudflare.com/sign-up and create a free account.
2. Go to **Storage & databases > R2**.
3. Create bucket named `vcisbgb-issue-files`.
4. Install Node.js, then run in this folder:

```powershell
npm install
npx wrangler login
npx wrangler secret put SUPABASE_URL
npx wrangler secret put SUPABASE_ANON_KEY
npx wrangler secret put SUPABASE_SERVICE_ROLE_KEY
npx wrangler deploy
```

Use these values when prompted:

- `SUPABASE_URL`: `https://tkaihqvegcisjmdpncqj.supabase.co`
- `SUPABASE_ANON_KEY`: Supabase publishable/anon key
- `SUPABASE_SERVICE_ROLE_KEY`: Supabase service-role key, never put it in GitHub or React
## 2. Connect GitHub Pages

Copy the Worker URL into a GitHub repository secret named `VITE_R2_API_URL`.
The existing Pages workflow will then build the web app with R2 upload/download enabled.

## Security

The Worker checks the Supabase login token and the user's `can_edit` or `can_download` permission before accessing R2. Do not expose `SUPABASE_SERVICE_ROLE_KEY` to the browser.
