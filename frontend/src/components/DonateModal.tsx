"use client";

import { useState } from "react";
import { useDonate } from "@/hooks/useSoroban";
import { Campaign } from "@/lib/soroban";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { Loader2 } from "lucide-react";

export function DonateModal({ campaign }: { campaign: Campaign }) {
  const [amount, setAmount] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const donate = useDonate();

  const handleDonate = async () => {
    if (!amount || isNaN(Number(amount))) {
      toast.error("Please enter a valid amount");
      return;
    }

    try {
      await donate.mutateAsync({
        campaignId: campaign.id,
        amount,
      });
      toast.success("Thank you for your donation!");
      setIsOpen(false);
      setAmount("");
    } catch (e: any) {
      toast.error(e.message || "Failed to donate");
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger asChild>
        <Button className="flex-1">Donate Now</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Donate to {campaign.title}</DialogTitle>
          <DialogDescription>
            Enter the amount of XLM you wish to contribute to this relief campaign.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="grid gap-2">
            <Label htmlFor="amount">Amount (XLM)</Label>
            <Input
              id="amount"
              type="number"
              placeholder="10.0"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => setIsOpen(false)}>
            Cancel
          </Button>
          <Button onClick={handleDonate} disabled={donate.isPending}>
            {donate.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Confirm Donation
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
