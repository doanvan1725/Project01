-- Run this script once in Supabase SQL Editor.
create extension if not exists "uuid-ossp";

create type public.user_role as enum ('admin', 'editor', 'viewer');
create type public.issue_status as enum ('Mới tạo', 'Đang xử lý', 'Đã trả lời');

create table public.profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  full_name text not null default '',
  role public.user_role not null default 'viewer',
  can_view boolean not null default true,
  can_edit boolean not null default false,
  can_delete boolean not null default false,
  can_download boolean not null default false,
  created_at timestamptz not null default now()
);

create table public.issues (
  id uuid primary key default uuid_generate_v4(),
  creator_name text not null,
  category text not null,
  content text not null,
  attachments jsonb not null default '[]'::jsonb,
  created_at timestamptz not null default now(),
  reply text not null default '',
  responder_name text not null default '',
  replied_at timestamptz,
  status public.issue_status not null default 'Mới tạo',
  created_by uuid references auth.users(id) on delete set null
);

alter table public.profiles enable row level security;
alter table public.issues enable row level security;

create or replace function public.current_user_role()
returns public.user_role language sql stable security definer set search_path = public
as $$ select role from public.profiles where id = auth.uid() $$;

create policy "Users can read own profile" on public.profiles for select to authenticated using (id = auth.uid() or public.current_user_role() = 'admin');
create policy "Admins can update profiles" on public.profiles for update to authenticated using (public.current_user_role() = 'admin');
create policy "Authenticated users can read issues" on public.issues for select to authenticated using (true);
create policy "Editors can create issues" on public.issues for insert to authenticated with check (public.current_user_role() in ('admin', 'editor'));
create policy "Editors can update issues" on public.issues for update to authenticated using (public.current_user_role() in ('admin', 'editor')) with check (public.current_user_role() in ('admin', 'editor'));
create policy "Admins can delete issues" on public.issues for delete to authenticated using (public.current_user_role() = 'admin');

insert into storage.buckets (id, name, public) values ('issue-files', 'issue-files', true) on conflict (id) do nothing;
create policy "Authenticated users can upload issue files" on storage.objects for insert to authenticated with check (bucket_id = 'issue-files' and public.current_user_role() in ('admin', 'editor'));
create policy "Anyone can read issue files" on storage.objects for select using (bucket_id = 'issue-files');
create policy "Admins can delete issue files" on storage.objects for delete to authenticated using (bucket_id = 'issue-files' and public.current_user_role() = 'admin');

-- After creating the first account, promote it from viewer to admin:
-- update public.profiles set role = 'admin' where id = 'YOUR_AUTH_USER_UUID';

create or replace function public.handle_new_user()
returns trigger language plpgsql security definer set search_path = public
as $$ begin insert into public.profiles (id, full_name) values (new.id, coalesce(new.raw_user_meta_data->>'full_name', new.email)); return new; end; $$;
create trigger on_auth_user_created after insert on auth.users for each row execute procedure public.handle_new_user();

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
create policy "Authenticated users can read issue versions" on public.issue_versions for select to authenticated using (true);
create policy "Editors can create issue versions" on public.issue_versions for insert to authenticated with check (public.current_user_role() in ('admin', 'editor'));
