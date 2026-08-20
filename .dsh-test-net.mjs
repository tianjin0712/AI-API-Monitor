const u = process.argv[2];
fetch(u)
  .then((r) => {
    if (!r.ok) throw new Error('HTTP ' + r.status);
    return r.json();
  })
  .then((j) => {
    const latest = j && j['dist-tags'] && j['dist-tags'].latest;
    if (latest) console.log('OK latest=' + latest);
    else if (j && j.total_count !== undefined) console.log('OK gh total=' + j.total_count);
    else console.log('OK json keys=' + Object.keys(j).slice(0, 5).join(','));
  })
  .catch((e) => {
    console.error('ERR: ' + e.message);
    process.exit(1);
  });
