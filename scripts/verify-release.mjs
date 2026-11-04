// Read-only release gate: GitHub must verify both source commit and annotated tag.
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
const repo = process.env.GITHUB_REPOSITORY;
const tag = process.env.GITHUB_REF_NAME;
if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo ?? '') || !/^v\d+\.\d+\.\d+$/.test(tag ?? '')) throw new Error('Invalid repository or release tag');
const headers = { Authorization: `Bearer ${process.env.GITHUB_TOKEN}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' };
async function get(path) {
  const r = await fetch(`https://api.github.com/repos/${repo}/${path}`, { headers });
  if (!r.ok) throw new Error(`GitHub release verification failed (${r.status})`);
  return r.json();
}
const ref = await get(`git/ref/tags/${encodeURIComponent(tag)}`);
if (ref.object.type !== 'tag') throw new Error('Release requires an annotated, signed tag; lightweight tags are refused');
const signedTag = await get(`git/tags/${ref.object.sha}`);
if (!signedTag.verification?.verified || signedTag.object.type !== 'commit') throw new Error('GitHub did not verify annotated tag signature');
const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
if (head !== signedTag.object.sha) throw new Error('Checked-out commit differs from signed release tag');
const history = execFileSync('git', ['rev-list', 'HEAD'], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }).trim().split('\n');
for (const sha of history) {
  const commit = await get(`commits/${sha}`);
  if (!commit.commit.verification?.verified) throw new Error(`GitHub did not verify a commit signature: ${sha}`);
}
const cargoVersion = readFileSync('Cargo.toml', 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (`v${cargoVersion}` !== tag) throw new Error('Cargo version and signed tag differ');
console.log(`Verified signatures on all reachable source commits and annotated release tag ${tag}`);
