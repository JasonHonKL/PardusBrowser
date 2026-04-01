import { chromium } from 'playwright';

const BASE_URL = process.env.BENCH_URL || 'http://127.0.0.1:18899';
const ITERATIONS = parseInt(process.env.ITERATIONS || '10', 10);
const PAGE = process.env.BENCH_PAGE || 'realistic.html';

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

  for (let i = 0; i < iterations; i++) {
    const context = await browser.newContext();
    const page = await context.newPage();

    const start = performance.now();
    await page.goto(url, { waitUntil: 'networkidle', timeout: 15000 });
    const end = performance.now();

    times.push(Math.round(end - start));
    await context.close();
  }

  times.sort((a, b) => a - b);
  const sum = times.reduce((a, b) => a + b, 0);

  // DOM stats
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(url, { waitUntil: 'networkidle', timeout: 15000 });
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
  await context.close();

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
  process.stderr.write('Launching Chromium (Playwright)...\n');
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
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

  const { version } = JSON.parse(await import('fs').then(f => f.promises.readFile(new URL('./node_modules/playwright/package.json', import.meta.url), 'utf-8')).catch(() => '{}'));

  const output = {
    tool: 'playwright',
    version: version || 'unknown',
    iterations: ITERATIONS,
    pages: results
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
