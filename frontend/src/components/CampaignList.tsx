"use client";

import { useRecentCampaigns } from "@/hooks/useSoroban";
import { CampaignCard } from "@/components/CampaignCard";
import { Skeleton } from "@/components/ui/skeleton";

export function CampaignList() {
  const { data: campaigns, isLoading, error } = useRecentCampaigns();

  if (isLoading) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-[300px] rounded-xl bg-muted animate-pulse" />
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-12 text-destructive">
        Failed to load campaigns. Please ensure you are on Testnet.
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      {campaigns?.map((campaign) => (
        <CampaignCard key={campaign.id.toString()} campaign={campaign} />
      ))}
      {campaigns?.length === 0 && (
        <div className="col-span-full text-center py-12 text-muted-foreground">
          No campaigns found. Be the first to create one!
        </div>
      )}
    </div>
  );
}
