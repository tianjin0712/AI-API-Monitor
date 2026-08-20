import { writeFileSync } from 'node:fs';

const out = {};

async function getJSON(url, headers = {}) {
  const r = await fetch(url, { headers: { 'user-agent': 'dsh-agent', accept: 'application/vnd.github+json', ...headers } });
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}

try {
  // dshmarket npm metadata
  const market = await getJSON('https://registry.npmjs.org/dshmarket');
  out.market = {
    latest: market['dist-tags'],
    versions: Object.keys(market.versions).slice(-15),
    latestManifest: market.versions[market['dist-tags'].latest],
    repo: market.repository,
    description: market.description,
    time: market.time,
  };

  // dsh-find-plugin metadata
  const find = await getJSON('https://registry.npmjs.org/dsh-find-plugin');
  out.find = {
    latest: find['dist-tags'],
    latestManifest: find.versions[find['dist-tags'].latest],
    repo: find.repository,
    description: find.description,
  };
} catch (e) {
  out.npmError = String(e);
}

try {
  // dsh-plugin topic repos, sorted by stars
  const gh = await getJSON('https://api.github.com/search/repositories?q=topic:dsh-plugin&sort=stars&order=desc&per_page=100');
  out.ghTotal = gh.total_count;
  out.ghRepos = (gh.items || []).map((r) => ({
    full_name: r.full_name,
    stars: r.stargazers_count,
    pushed: r.pushed_at,
    desc: (r.description || '').slice(0, 120),
    topics: r.topics,
  }));
} catch (e) {
  out.ghError = String(e);
}

writeFileSync('E:/Project/AI API Monitor/.dsh-research.json', JSON.stringify(out, null, 2));
console.log('DONE market latest=' + (out.market?.latest?.latest) + ' find latest=' + (out.find?.latest?.latest) + ' ghTotal=' + out.ghTotal);
