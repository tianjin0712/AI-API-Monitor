async function probe(url) {
  try {
    const r = await fetch(url, { headers: { 'user-agent': 'dsh-agent' } });
    const text = await r.text();
    console.log('=== ' + url + ' ===');
    console.log('status=' + r.status + ' contentType=' + (r.headers.get('content-type') || '') + ' len=' + text.length);
    console.log(text.slice(0, 600));
    console.log('');
  } catch (e) {
    console.log('=== ' + url + ' === FAILED: ' + e.message);
    console.log('');
  }
}
await probe('https://awesome-dsh-plugin.com/plugins.json');
await probe('https://awesome-dsh-plugin.com/');
await probe('https://raw.githubusercontent.com/awesome-dsh-plugin/awesome-dsh-plugin/main/README.md');
