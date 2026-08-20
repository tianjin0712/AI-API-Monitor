import { writeFileSync } from 'node:fs';

async function getJSON(url, headers = {}) {
  const r = await fetch(url, { headers: { 'user-agent': 'dsh-agent', accept: 'application/json', ...headers } });
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}
async function getText(url) {
  const r = await fetch(url, { headers: { 'user-agent': 'dsh-agent' } });
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.text();
}

const out = {};
try {
  out.plugins = await getJSON('https://awesome-dsh-plugin.com/plugins.json');
  out.pluginsCount = Array.isArray(out.plugins) ? out.plugins.length : 'not-array';
} catch (e) { out.pluginsError = String(e); }

try {
  out.awesomeReadme = await getText('https://raw.githubusercontent.com/awesome-dsh-plugin/awesome-dsh-plugin/main/README.md');
} catch (e) {
  try {
    out.awesomeReadme = await getText('https://raw.githubusercontent.com/awesome-dsh-plugin/awesome-dsh-plugin/master/README.md');
  } catch (e2) { out.readmeError = String(e2); }
}

writeFileSync('E:/Project/AI API Monitor/.dsh-registry.json', JSON.stringify(out, null, 2));
console.log('pluginsCount=' + out.pluginsCount + ' readmeLen=' + (out.awesomeReadme ? out.awesomeReadme.length : 'ERR ' + out.readmeError));
