import type { AvailabilityLevel } from "@/types/runtime";

export interface AccountUsage {
  accountId: string;
  availabilityStatus: string;
  usedPercent: number | null;
  windowMinutes: number | null;
  resetsAt: number | null;
  secondaryUsedPercent: number | null;
  secondaryWindowMinutes: number | null;
  secondaryResetsAt: number | null;
  creditsJson: string | null;
  capturedAt: number | null;
}

export interface Account {
  id: string;
  name: string;
  group: string;
  priority: number;
  preferred: boolean;
  label: string;
  groupName: string;
  sort: number;
  status: string;
  statusReason: string;
  planType: string | null;
  planTypeRaw: string | null;
  note: string | null;
  tags: string[];
  isAvailable: boolean;
  isLowQuota: boolean;
  lastRefreshAt: number | null;
  availabilityText: string;
  availabilityLevel: AvailabilityLevel;
  primaryRemainPercent: number | null;
  secondaryRemainPercent: number | null;
  usage: AccountUsage | null;
}

export interface AccountListResult {
  items: Account[];
  total: number;
  page: number;
  pageSize: number;
}

export interface ManagedTeam {
  id: string;
  sourceAccountId: string;
  sourceAccountLabel: string | null;
  sourceAccountStatus: string | null;
  teamAccountId: string | null;
  teamName: string | null;
  planType: string | null;
  subscriptionPlan: string | null;
  status: string;
  currentMembers: number;
  pendingInvites: number;
  maxMembers: number;
  occupiedSlots: number;
  expiresAt: number | null;
  lastSyncAt: number | null;
  createdAt: number | null;
  updatedAt: number | null;
}

export interface ManagedTeamMember {
  email: string;
  name: string | null;
  role: string | null;
  status: string;
  userId: string | null;
  addedAt: number | null;
}

export interface ManagedTeamMembersResult {
  teamId: string;
  items: ManagedTeamMember[];
}

export interface ManagedTeamInviteResult {
  invitedCount: number;
  teamId: string;
  message: string;
}

export interface UsageAggregateSummary {
  primaryBucketCount: number;
  primaryKnownCount: number;
  primaryUnknownCount: number;
  primaryRemainPercent: number | null;
  secondaryBucketCount: number;
  secondaryKnownCount: number;
  secondaryUnknownCount: number;
  secondaryRemainPercent: number | null;
}
