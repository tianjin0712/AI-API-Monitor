import { readFileSync, writeFileSync } from 'node:fs';

const raw = JSON.parse(readFileSync('E:/Project/AI API Monitor/.dsh-registry.json', 'utf8'));
// 重新拉取完整 plugins.json(上次存的是包装对象)
const r = await fetch('https://awesome-dsh-plugin.com/plugins.json', { headers: { 'user-agent': 'dsh-agent' } });
const data = await r.json();

const summary = {
  name: data.name,
  updated: data.updated,
  count: data.count,
  categories: Object.keys(data.categories || {}).map((k) => ({ key: k, en: data.categories[k]?.en, zh: data.categories[k]?.zh })),
  topLevelKeys: Object.keys(data),
};

// 探查插件列表字段结构
const listKeys = Object.keys(data).filter((k) => k !== 'categories' && k !== 'name' && k !== 'url' && k !== 'source' && k !== 'updated' && k !== 'count');
summary.listKeys = listKeys;

// 找到插件数组并抽样
let sample = null;
for (const k of listKeys) {
  const v = data[k];
  if (Array.isArray(v) && v.length > 0) {
    sample = { key: k, len: v.length, firstItem: v[0] };
    break;
  }
}
summary.sample = sample;

writeFileSync('E:/Project/AI API Monitor/.dsh-registry-summary.json', JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
