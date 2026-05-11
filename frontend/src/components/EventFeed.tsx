"use client";

import { useEvents } from "@/hooks/useSoroban";
import { fromStroops } from "@/lib/soroban";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Activity, ArrowUpRight, Megaphone, Trophy } from "lucide-react";
import { formatDistanceToNow } from "date-fns";

export function EventFeed() {
  const { data: events, isLoading } = useEvents();

  if (isLoading) {
    return <Activity className="animate-spin mx-auto my-8 text-muted-foreground" />;
  }

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle className="text-lg flex items-center gap-2">
          <Activity className="w-4 h-4 text-primary" /> Recent Activity
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        {events?.map((event: any) => (
          <div key={event.id} className="flex gap-4 items-start">
            <div className={`mt-1 p-2 rounded-full ${
              event.topic === 'received' ? 'bg-green-500/10' :
              event.topic === 'created' ? 'bg-blue-500/10' :
              'bg-purple-500/10'
            }`}>
              {event.topic === 'received' && <ArrowUpRight className="w-3 h-3 text-green-500" />}
              {event.topic === 'created' && <Megaphone className="w-3 h-3 text-blue-500" />}
              {event.topic === 'claimed' && <Trophy className="w-3 h-3 text-purple-500" />}
            </div>
            <div className="flex-1 space-y-1">
              <p className="text-sm">
                {event.topic === 'received' && (
                  <>
                    <span className="font-bold">{fromStroops(event.data[2])} XLM</span> donated to Campaign #{event.data[0].toString()}
                  </>
                )}
                {event.topic === 'created' && (
                  <>
                    New campaign created with a target of <span className="font-bold">{fromStroops(event.data[3])} XLM</span>
                  </>
                )}
                {event.topic === 'claimed' && (
                  <>
                    <span className="font-bold">{fromStroops(event.data[3])} XLM</span> claimed by beneficiary
                  </>
                )}
              </p>
              <div className="flex items-center gap-2 text-[10px] text-muted-foreground uppercase font-bold tracking-wider">
                <span>{event.topic}</span>
                <span>•</span>
                <span>Ledger {event.ledger}</span>
              </div>
            </div>
          </div>
        ))}
        {!events?.length && (
          <div className="text-center py-8 text-muted-foreground text-sm">
            Waiting for on-chain events...
          </div>
        )}
      </CardContent>
    </Card>
  );
}
