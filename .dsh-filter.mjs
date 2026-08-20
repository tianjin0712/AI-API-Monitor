import { readFileSync, writeFileSync } from 'node:fs';

const r = await fetch('https://awesome-dsh-plugin.com/plugins.json', { headers: { 'user-agent': 'dsh-agent' } });
const data = await r.json();
const plugins = data.plugins || [];

// 重点分类
const focus = ['git', 'dev', 'security', 'remote', 'market', 'skill', 'usage', 'model', 'tools', 'workflow'];

const byCat = {};
for (const p of plugins) {
  (byCat[p.category] ||= []).push(p);
}

const result = { categories: {} };
for (const cat of focus) {
  const list = (byCat[cat] || [])
    .map((p) => ({
      name: p.name, npm: p.npm, stars: p.stars, downloads: p.downloads,
      owner: p.owner, desc: (p.description?.en || '').slice(0, 100), url: p.url,
    }))
    .sort((a, b) => (b.downloads || 0) - (a.downloads || 0));
  result.categories[cat] = list;
}

// 同时输出所有分类的插件总数
result.catCounts = Object.fromEntries(Object.entries(byCat).map(([k, v]) => [k, v.length]));

writeFileSync('E:/Project/AI API Monitor/.dsh-focus.json', JSON.stringify(result, null, 2));
console.log('catCounts:', JSON.stringify(result.catCounts));
for (const cat of focus) {
  console.log('--- ' + cat + ' (' + result.categories[cat].length + ') top8 ---');
  for (const p of result.categories[cat].slice(0, 8)) {
    console.log(`  ${p.name}  dl=${p.downloads} star=${p.stars}  ${p.desc.slice(0, 70)}`);
  }
}
