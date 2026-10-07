import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  // Font availability should not block checks of the site's own theme assets.
  await page.route("https://fonts.googleapis.com/**", (route) => route.abort());
  await page.route("https://fonts.gstatic.com/**", (route) => route.abort());
});

for (const { path, heading, screenshot } of [
  { path: "/docs/", heading: "Sqruff", screenshot: "docs-overview.png" },
  {
    path: "/docs/getting-started/installation/",
    heading: "Installation",
    screenshot: "docs-installation.png",
  },
]) {
  test(`docs load their theme assets at ${path}`, async ({ page }) => {
    const stylesheet = page.waitForResponse((response) =>
      new URL(response.url()).pathname.startsWith("/docs/assets/stylesheets/"),
    );
    const script = page.waitForResponse((response) =>
      new URL(response.url()).pathname.startsWith(
        "/docs/assets/javascripts/bundle.",
      ),
    );

    await page.goto(path, { waitUntil: "domcontentloaded" });

    const [css, js] = await Promise.all([stylesheet, script]);
    expect.soft(css.ok(), `Stylesheet failed: ${css.url()}`).toBe(true);
    expect.soft(css.headers()["content-type"]).toContain("text/css");
    expect.soft(js.ok(), `Script failed: ${js.url()}`).toBe(true);
    expect.soft(js.headers()["content-type"]).toMatch(/javascript/);

    const title = page.getByRole("heading", { level: 1 });
    await expect(title).toBeVisible();
    await expect(title).toContainText(heading);
    // Keep native scrollbar preferences from changing the available width.
    await page.addStyleTag({
      content: `
        * { scrollbar-width: none !important; scrollbar-gutter: auto !important; }
        ::-webkit-scrollbar { display: none !important; }
      `,
    });
    await expect(page).toHaveScreenshot(screenshot);
  });
}
