import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";
import ts from "../node_modules/typescript/lib/typescript.js";

const appsRoot = path.resolve(import.meta.dirname, "..");
const sourcePath = path.join(
  appsRoot,
  "src",
  "app",
  "teams",
  "teams-page-helpers.ts",
);

async function loadTeamsPageHelpers() {
  const source = await fs.readFile(sourcePath, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: sourcePath,
  });

  const tempDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "codexmanager-teams-page-helpers-"),
  );
  const tempFile = path.join(tempDir, "teams-page-helpers.mjs");
  await fs.writeFile(tempFile, compiled.outputText, "utf8");
  return import(pathToFileURL(tempFile).href);
}

const helpers = await loadTeamsPageHelpers();

const teams = [
  {
    id: "team-1",
    sourceAccountId: "acct-plus-01",
    sourceAccountLabel: "主账号 A",
    teamAccountId: "team-acct-001",
    teamName: "Alpha Team",
    status: "active",
    currentMembers: 3,
    pendingInvites: 1,
    maxMembers: 5,
    occupiedSlots: 4,
  },
  {
    id: "team-2",
    sourceAccountId: "acct-pro-02",
    sourceAccountLabel: "运营号 B",
    teamAccountId: "team-acct-002",
    teamName: "Beta Squad",
    status: "full",
    currentMembers: 5,
    pendingInvites: 0,
    maxMembers: 5,
    occupiedSlots: 5,
  },
  {
    id: "team-3",
    sourceAccountId: "acct-expired-03",
    sourceAccountLabel: null,
    teamAccountId: "team-acct-003",
    teamName: "Gamma Org",
    status: "expired",
    currentMembers: 1,
    pendingInvites: 2,
    maxMembers: 5,
    occupiedSlots: 3,
  },
];

test("filterManagedTeams 按搜索词匹配团队名、母号和 account id", () => {
  assert.deepEqual(
    helpers
      .filterManagedTeams(teams, { search: "运营号", status: "all" })
      .map((team) => team.id),
    ["team-2"],
  );

  assert.deepEqual(
    helpers
      .filterManagedTeams(teams, { search: "team-acct-003", status: "all" })
      .map((team) => team.id),
    ["team-3"],
  );

  assert.deepEqual(
    helpers
      .filterManagedTeams(teams, { search: " alpha ", status: "all" })
      .map((team) => team.id),
    ["team-1"],
  );
});

test("filterManagedTeams 叠加状态筛选并忽略大小写空白", () => {
  assert.deepEqual(
    helpers
      .filterManagedTeams(teams, { search: "", status: " active " })
      .map((team) => team.id),
    ["team-1"],
  );

  assert.deepEqual(
    helpers
      .filterManagedTeams(teams, { search: "team", status: "full" })
      .map((team) => team.id),
    ["team-2"],
  );
});

test("buildManagedTeamStats 汇总团队数量、活跃数、席位与邀请数", () => {
  assert.deepEqual(helpers.buildManagedTeamStats(teams), {
    totalTeams: 3,
    activeTeams: 1,
    occupiedSlots: 12,
    pendingInvites: 3,
  });
});

test("mergeManagedTeamInviteMembers 立即把新邀请并入本地成员缓存，避免重复发送", () => {
  const current = {
    teamId: "team-1",
    items: [
      {
        email: "joined@example.com",
        name: "Joined",
        role: "standard-user",
        status: "joined",
        userId: "user-1",
        addedAt: 100,
      },
      {
        email: "existing-invite@example.com",
        name: null,
        role: "standard-user",
        status: "invited",
        userId: null,
        addedAt: 200,
      },
    ],
  };

  const next = helpers.mergeManagedTeamInviteMembers(
    current,
    {
      teamId: "team-1",
      invited: ["existing-invite@example.com"],
      pendingSync: ["new-invite@example.com"],
    },
    "team-1",
    500,
  );

  assert.equal(next.items.length, 3);
  assert.deepEqual(
    next.items.map((item) => item.email),
    [
      "existing-invite@example.com",
      "joined@example.com",
      "new-invite@example.com",
    ],
  );
  assert.deepEqual(
    next.items.find((item) => item.email === "new-invite@example.com"),
    {
      email: "new-invite@example.com",
      name: null,
      role: "standard-user",
      status: "invited",
      userId: null,
      addedAt: 500,
    },
  );
});

test("removeManagedTeamMemberFromCache 会按邮箱移除成员或邀请", () => {
  const current = {
    teamId: "team-1",
    items: [
      {
        email: "keep@example.com",
        name: null,
        role: "standard-user",
        status: "joined",
        userId: "user-keep",
        addedAt: 100,
      },
      {
        email: "remove@example.com",
        name: null,
        role: "standard-user",
        status: "invited",
        userId: null,
        addedAt: 200,
      },
    ],
  };

  const next = helpers.removeManagedTeamMemberFromCache(current, {
    email: " remove@example.com ",
  });

  assert.deepEqual(next.items.map((item) => item.email), ["keep@example.com"]);
});
