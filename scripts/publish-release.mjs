// CI-only: stage a NEW draft, upload all immutable assets, then publish atomically at release level.
import { readFileSync, readdirSync } from 'node:fs';
const repo = process.env.GITHUB_REPOSITORY;
const tag = process.env.GITHUB_REF_NAME;
if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo ?? '') || !/^v\d+\.\d+\.\d+$/.test(tag ?? '')) throw new Error('Invalid release context');
const headers = { Authorization: `Bearer ${process.env.GITHUB_TOKEN}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' };
async function api(path, options = {}) {
  const r = await fetch(`https://api.github.com/repos/${repo}/${path}`, { ...options, headers: { ...headers, ...options.headers } });
  if (!r.ok) throw new Error(`GitHub release API failed (${r.status}); an unfinished draft may need inspection`);
  return r.json();
}
// List releases with the authenticated push-capable token: this also includes drafts.
// A by-tag 404 alone cannot rule out a leftover draft from a failed run.
for (let page = 1; ; page++) {
  const releases = await api(`releases?per_page=100&page=${page}`);
  if (!Array.isArray(releases)) throw new Error('Unexpected release inventory response');
  if (releases.some(r => r.tag_name === tag)) throw new Error('A draft or published release exists; inspect it explicitly, never overwrite');
  if (releases.length < 100) break;
}
const files = readdirSync('dist').filter(n => n.endsWith('.tar.gz') || ['SHA256SUMS', 'install.sh', 'install.ps1'].includes(n)).sort();
if (files.filter(n => n.endsWith('.tar.gz')).length !== 5 || !files.includes('SHA256SUMS')) throw new Error('Expected all five native release bundles and checksums');
const release = await api('releases', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ tag_name: tag, name: tag, draft: true, generate_release_notes: true }) });
for (const name of files) {
  if (!/^[A-Za-z0-9._-]+$/.test(name)) throw new Error('Invalid asset filename');
  const url = new URL(release.upload_url.split('{')[0]);
  if (url.protocol !== 'https:' || url.hostname !== 'uploads.github.com') throw new Error('Unexpected upload destination');
  url.searchParams.set('name', name);
  const r = await fetch(url, { method: 'POST', headers: { ...headers, 'Content-Type': 'application/octet-stream' }, body: readFileSync(`dist/${name}`) });
  if (!r.ok) throw new Error(`Asset upload failed (${r.status}); draft retained for inspection`);
}
await api(`releases/${release.id}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ draft: false }) });
console.log(`Published ${tag} with ${files.length} verified-build assets`);
