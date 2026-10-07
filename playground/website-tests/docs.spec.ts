import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  // Font availability should not block checks of the site's own theme assets.
  await page.route("https://fonts.googleapis.com/**", (route) => route.abort());
  await page.route("https://fonts.gstatic.com/**", (route) => route.abort());
});

for (const { path, heading } of [
  { path: "/docs/", heading: "Sqruff" },
  { path: "/docs/getting-started/installation/", heading: "Installation" },
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
    expect(css.ok(), `Stylesheet failed: ${css.url()}`).toBe(true);
    expect(css.headers()["content-type"]).toContain("text/css");
    expect(js.ok(), `Script failed: ${js.url()}`).toBe(true);
    expect(js.headers()["content-type"]).toMatch(/javascript/);

    const title = page.getByRole("heading", { level: 1 });
    await expect(title).toBeVisible();
    await expect(title).toContainText(heading);
    // Check that the browser actually applied the theme, not just served HTML.
    await expect(page.locator(".md-header")).toHaveCSS("position", "sticky");
    await expect(page.locator(".md-sidebar--primary")).toBeVisible();
  });
}
