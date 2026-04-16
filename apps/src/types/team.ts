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
  skippedCount: number;
  teamId: string;
  invited: string[];
  alreadyJoined: string[];
  alreadyInvited: string[];
  pendingSync: string[];
  message: string;
}

export interface ManagedTeamMutationResult {
  teamId: string;
  message: string;
}
