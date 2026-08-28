-- Run this migration after the original schema.sql.
create table if not exists public.issue_versions (
  id uuid primary key default uuid_generate_v4(),
  issue_id uuid not null references public.issues(id) on delete cascade,
  version_number integer not null,
  attachments jsonb not null default '[]'::jsonb,
  note text not null default '',
  created_at timestamptz not null default now(),
  created_by uuid references auth.users(id) on delete set null,
  unique (issue_id, version_number)
);

alter table public.issue_versions enable row level security;
drop policy if exists "Authenticated users can read issue versions" on public.issue_versions;
drop policy if exists "Editors can create issue versions" on public.issue_versions;
create policy "Authenticated users can read issue versions" on public.issue_versions for select to authenticated using (true);
create policy "Editors can create issue versions" on public.issue_versions for insert to authenticated with check (public.current_user_role() in ('admin', 'editor'));

-- Backfill the current attachment of existing issues as version 1.
insert into public.issue_versions (issue_id, version_number, attachments)
select id, 1, attachments from public.issues
where jsonb_array_length(attachments) > 0
  and not exists (select 1 from public.issue_versions v where v.issue_id = issues.id);
