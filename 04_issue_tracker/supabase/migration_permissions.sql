-- Run after schema.sql and migration_versions.sql.
alter table public.profiles add column if not exists can_view boolean not null default true;
alter table public.profiles add column if not exists can_edit boolean not null default false;
alter table public.profiles add column if not exists can_delete boolean not null default false;
alter table public.profiles add column if not exists can_download boolean not null default false;

update public.profiles set can_edit = true, can_delete = true, can_download = true where role = 'admin';
update public.profiles set can_edit = true, can_download = true where role = 'editor';

create or replace function public.current_user_can(permission text)
returns boolean language sql stable security definer set search_path = public
as $$ select case permission
  when 'view' then can_view
  when 'edit' then can_edit
  when 'delete' then can_delete
  when 'download' then can_download
  else false end
from public.profiles where id = auth.uid() $$;

drop policy if exists "Authenticated users can read issues" on public.issues;
drop policy if exists "Editors can create issues" on public.issues;
drop policy if exists "Editors can update issues" on public.issues;
drop policy if exists "Admins can delete issues" on public.issues;
create policy "Users with view permission can read issues" on public.issues for select to authenticated using (public.current_user_can('view'));
create policy "Users with edit permission can create issues" on public.issues for insert to authenticated with check (public.current_user_can('edit'));
create policy "Users with edit permission can update issues" on public.issues for update to authenticated using (public.current_user_can('edit')) with check (public.current_user_can('edit'));
create policy "Users with delete permission can delete issues" on public.issues for delete to authenticated using (public.current_user_can('delete'));

drop policy if exists "Anyone can read issue files" on storage.objects;
create policy "Users with download permission can read issue files" on storage.objects for select to authenticated using (bucket_id = 'issue-files' and public.current_user_can('download'));
