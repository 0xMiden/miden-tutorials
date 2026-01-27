import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

const tutorialTimeoutMs = 10 * 60 * 1000;

const tutorials = [
  {
    name: "createMintConsume",
    testId: "tutorial-createMintConsume",
  },
  {
    name: "multiSendWithDelegatedProver",
    testId: "tutorial-multiSendWithDelegatedProver",
  },
  {
    name: "incrementCounterContract",
    testId: "tutorial-incrementCounterContract",
  },
  {
    name: "unauthenticatedNoteTransfer",
    testId: "tutorial-unauthenticatedNoteTransfer",
  },
  {
    name: "foreignProcedureInvocation",
    testId: "tutorial-foreignProcedureInvocation",
  },
] as const;

const runTutorial = async (
  page: Page,
  tutorialName: string,
  testId: string,
) => {
  const consoleErrors: string[] = [];

  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(`[console.error] ${msg.text()}`);
    }
  });
  page.on("pageerror", (err) => {
    consoleErrors.push(`[pageerror] ${err.message}`);
  });

  await page.goto("/");
  await page.getByTestId(testId).click();

  await page.waitForFunction(
    (name) => {
      const status = (window as Window & {
        __tutorialStatus?: Record<string, { state: string; error?: string }>;
      }).__tutorialStatus?.[name];
      return status?.state === "passed" || status?.state === "failed";
    },
    tutorialName,
    { timeout: tutorialTimeoutMs },
  );

  const status = await page.evaluate((name) => {
    const win = window as Window & {
      __tutorialStatus?: Record<string, { state: string; error?: string }>;
    };
    return win.__tutorialStatus?.[name] ?? null;
  }, tutorialName);

  if (consoleErrors.length > 0) {
    throw new Error(
      `Console errors detected during ${tutorialName}:\n${consoleErrors.join(
        "\n",
      )}`,
    );
  }

  expect(status).not.toBeNull();
  if (status?.state === "failed") {
    throw new Error(
      `Tutorial ${tutorialName} failed: ${status.error ?? "unknown error"}`,
    );
  }
  expect(status?.state).toBe("passed");
};

for (const tutorial of tutorials) {
  test(tutorial.name, async ({ page }) => {
    test.setTimeout(tutorialTimeoutMs);
    await runTutorial(page, tutorial.name, tutorial.testId);
  });
}
