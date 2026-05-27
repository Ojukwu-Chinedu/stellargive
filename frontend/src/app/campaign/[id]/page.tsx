"use client";

import { useCampaign } from "@/hooks/useSoroban";
import { ShareButton } from "@/components/ShareButton";
import { Skeleton } from "@/components/ui/skeleton";
import { RecentDonations } from "@/components/RecentDonations";

export default function CampaignDetails({ params }: { params: { id: string } }) {
  const { data: campaign, isLoading } = useCampaign(BigInt(params.id));

  return (
    <div className="p-8 max-w-4xl mx-auto space-y-6">
      <div className="flex justify-between items-start">
        <h1 className="text-3xl font-bold">
          {isLoading ? (
            <Skeleton className="h-9 w-64" />
          ) : (
            campaign?.title || `Campaign #${params.id}`
          )}
        </h1>
        {campaign && <ShareButton campaign={campaign} />}
      </div>
    <div className="p-8">
      <div className="flex justify-between items-start">
        <h1 className="text-2xl font-bold">
          {isLoading ? <Skeleton className="h-8 w-64" /> : campaign?.title || `Campaign Details: ${params.id}`}
        </h1>
        {campaign && <ShareButton campaign={campaign} />}
      </div>
      
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8 pt-4">
        <div className="lg:col-span-2 space-y-6">
          {/* Main campaign info placeholder */}
          <div className="h-64 bg-muted/20 rounded-xl border border-dashed flex items-center justify-center text-muted-foreground text-sm">
            Campaign Content Area
          </div>
        </div>
        
        <div className="lg:col-span-1">
          <RecentDonations campaignId={BigInt(params.id)} />
        </div>
      </div>
    </div>
  );
}
