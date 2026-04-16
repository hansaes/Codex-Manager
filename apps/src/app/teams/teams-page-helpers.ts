export type ManagedTeamStatusFilter = "all" | "active" | "full" | "expired";

export interface ManagedTeamLike {
  id: string;
  sourceAccountId: string;
  sourceAccountLabel: string | null;
  teamAccountId: string | null;
  teamName: string | null;
  status: string;
  currentMembers: number;
  pendingInvites: number;
  maxMembers: number;
  occupiedSlots: number;
}

export interface ManagedTeamStats {
  totalTeams: number;
  activeTeams: number;
  occupiedSlots: number;
  pendingInvites: number;
}

export interface ManagedTeamMemberLike {
  email: string;
  name: string | null;
  role: string | null;
  status: string;
  userId: string | null;
  addedAt: number | null;
}

export interface ManagedTeamMembersLike {
  teamId: string;
  items: ManagedTeamMemberLike[];
}

function normalizeText(value: unknown): string {
  return String(value ?? "").trim().toLowerCase();
}

export function normalizeManagedTeamStatusFilter(
  value: string | null | undefined,
): ManagedTeamStatusFilter {
  const normalized = normalizeText(value);
  if (
    normalized === "active" ||
    normalized === "full" ||
    normalized === "expired"
  ) {
    return normalized;
  }
  return "all";
}

export function buildManagedTeamSearchText(team: ManagedTeamLike): string {
  return [
    team.teamName,
    team.sourceAccountLabel,
    team.sourceAccountId,
    team.teamAccountId,
  ]
    .map(normalizeText)
    .filter(Boolean)
    .join(" ");
}

export function filterManagedTeams<T extends ManagedTeamLike>(
  teams: readonly T[],
  filters: {
    search?: string | null;
    status?: string | null;
  },
): T[] {
  const search = normalizeText(filters.search);
  const statusFilter = normalizeManagedTeamStatusFilter(filters.status);

  return teams.filter((team) => {
    const matchesSearch =
      search.length === 0 || buildManagedTeamSearchText(team).includes(search);
    const matchesStatus =
      statusFilter === "all" || normalizeText(team.status) === statusFilter;

    return matchesSearch && matchesStatus;
  });
}

export function buildManagedTeamStats(
  teams: readonly ManagedTeamLike[],
): ManagedTeamStats {
  return teams.reduce<ManagedTeamStats>(
    (summary, team) => {
      summary.totalTeams += 1;
      if (normalizeText(team.status) === "active") {
        summary.activeTeams += 1;
      }
      summary.occupiedSlots += Number(team.occupiedSlots || 0);
      summary.pendingInvites += Number(team.pendingInvites || 0);
      return summary;
    },
    {
      totalTeams: 0,
      activeTeams: 0,
      occupiedSlots: 0,
      pendingInvites: 0,
    },
  );
}

function sortManagedTeamMembers<T extends ManagedTeamMemberLike>(items: readonly T[]): T[] {
  return [...items].sort((left, right) =>
    normalizeText(left.email).localeCompare(normalizeText(right.email)),
  );
}

export function mergeManagedTeamInviteMembers<
  T extends ManagedTeamMemberLike,
  TResult extends {
    teamId?: string | null;
    invited?: readonly string[] | null;
    pendingSync?: readonly string[] | null;
  },
>(
  current: ManagedTeamMembersLike | null | undefined,
  result: TResult,
  fallbackTeamId: string,
  addedAt: number,
): ManagedTeamMembersLike {
  const currentItems = current?.items ?? [];
  const knownEmails = new Set(
    currentItems.map((item) => normalizeText(item.email)).filter(Boolean),
  );
  const newEmails = [
    ...(result.invited ?? []),
    ...(result.pendingSync ?? []),
  ]
    .map(normalizeText)
    .filter(Boolean)
    .filter((email, index, values) => values.indexOf(email) === index)
    .filter((email) => !knownEmails.has(email));

  if (newEmails.length === 0) {
    return {
      teamId: current?.teamId || String(result.teamId || fallbackTeamId).trim(),
      items: sortManagedTeamMembers(currentItems),
    };
  }

  const optimisticItems = newEmails.map<ManagedTeamMemberLike>((email) => ({
    email,
    name: null,
    role: "standard-user",
    status: "invited",
    userId: null,
    addedAt,
  }));

  return {
    teamId: current?.teamId || String(result.teamId || fallbackTeamId),
    items: sortManagedTeamMembers([...currentItems, ...optimisticItems]),
  };
}

export function removeManagedTeamMemberFromCache<
  T extends ManagedTeamMemberLike,
  TCurrent extends ManagedTeamMembersLike | null | undefined,
>(
  current: TCurrent,
  member: { email?: string | null },
): ManagedTeamMembersLike {
  const targetEmail = normalizeText(member.email);
  return {
    teamId: current?.teamId || "",
    items: sortManagedTeamMembers(
      (current?.items ?? []).filter(
        (item) => normalizeText(item.email) !== targetEmail,
      ),
    ),
  };
}
