import puppeteer from 'puppeteer';

const BASE_URL = process.env.BENCH_URL || 'http://127.0.0.1:18899';
const ITERATIONS = parseInt(process.env.ITERATIONS || '10', 10);
const PAGE = process.env.PENCH_PAGE || 'realistic.html';
const URL = `${BASE_URL}/${PAGE}`;

const pages = process.env.ALL_PAGES === '1'
  ? [
      'simple.html', 'deep-nested.html', 'wide-dom.html',
      'interactive.html', 'semantic.html', 'content-heavy.html',
      'forms.html', 'nav-graph.html', 'realistic.html'
    ]
  : [PAGE];

function percentile(sorted, p) {
  const idx = Math.ceil(sorted.length * p / 100) - 1;
  return sorted[Math.max(0, Math.min(idx, sorted.length - 1))];
}

async function benchPage(browser, url) {
  const iterations = ITERATIONS;
  const times = [];
  const mems = [];

  for (let i = 0; i < iterations; i++) {
    const page = await browser.newPage();
    const client = await page.target().createCDPSession();
    await client.send('Performance.enable');

    const start = performance.now();
    await page.goto(url, { waitUntil: 'networkidle0', timeout: 15000 });
    const end = performance.now();

    const metrics = await page.metrics();
    const perf = await client.send('Performance.getMetrics');

    times.push(Math.round(end - start));
    mems.push(metrics.LayoutDuration + metrics.RecalcStyleCount + metrics.ScriptDuration);

    await page.close();
  }

  times.sort((a, b) => a - b);
  const sum = times.reduce((a, b) => a + b, 0);

  // DOM stats
  const page = await browser.newPage();
  await page.goto(url, { waitUntil: 'networkidle0', timeout: 15000 });
  const domStats = await page.evaluate(() => {
    const all = document.querySelectorAll('*');
    const interactive = document.querySelectorAll('a, button, input, select, textarea, [tabindex], [role="button"], [role="link"]');
    const headings = document.querySelectorAll('h1, h2, h3, h4, h5, h6');
    const links = document.querySelectorAll('a[href]');
    const forms = document.querySelectorAll('form');
    return {
      total_nodes: all.length,
      interactive: interactive.length,
      headings: headings.length,
      links: links.length,
      forms: forms.length,
      title: document.title
    };
  });
  await page.close();

  return {
    avg: Math.round(sum / iterations),
    min: times[0],
    max: times[times.length - 1],
    p50: percentile(times, 50),
    p99: percentile(times, 99),
    dom: domStats
  };
}

async function main() {
  console.error('Launching Chrome (Puppeteer)...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage']
  });

  const results = {};
  for (const p of pages) {
    const url = `${BASE_URL}/${p}`;
    process.stderr.write(`  ${p} ...`);
    const r = await benchPage(browser, url);
    results[p] = r;
    process.stderr.write(` avg:${r.avg}ms min:${r.min}ms max:${r.max}ms p50:${r.p50}ms p99:${r.p99}ms nodes:${r.dom.total_nodes}\n`);
  }

  await browser.close();

  const output = {
    tool: 'puppeteer',
    version: (await import('puppeteer/package.json', { assert: { type: 'json' } })).default.version,
    chromium: (await import('puppeteer/package.json', { assert: { type: 'json' } })).default.puppeteer?.chromium_revision || 'unknown',
    iterations: ITERATIONS,
    pages: results
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
