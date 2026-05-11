"use client";

import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import * as z from "zod";
import { useCreateCampaign } from "@/hooks/useSoroban";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";
import { useState } from "react";
import { Loader2, PlusCircle } from "lucide-react";

const formSchema = z.z.object({
  title: z.z.string().min(3).max(50),
  beneficiary: z.z.string().regex(/^G[A-Z0-9]{55}$/, "Invalid Stellar address"),
  targetAmount: z.z.string().refine((val) => !isNaN(Number(val)) && Number(val) > 0, "Amount must be positive"),
  deadlineDays: z.z.string().refine((val) => !isNaN(Number(val)) && Number(val) > 0, "Days must be positive"),
  acceptedToken: z.z.string().regex(/^C[A-Z0-9]{55}$|^G[A-Z0-9]{55}$/, "Invalid Token address"),
});

// For simplicity, we use native XLM (CDL... or dummy) but the contract requires a token address.
// On Testnet, native XLM is CDLZS3ZCDY7SF3SIVR6Y7I6SN636O27T7G5MKSUIU22ZS76E55WJIPZ4
const NATIVE_XLM = "CDLZS3ZCDY7SF3SIVR6Y7I6SN636O27T7G5MKSUIU22ZS76E55WJIPZ4";

export function CreateCampaignForm() {
  const [isOpen, setIsOpen] = useState(false);
  const createCampaign = useCreateCampaign();

  const form = useForm<z.z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      title: "",
      beneficiary: "",
      targetAmount: "",
      deadlineDays: "30",
      acceptedToken: NATIVE_XLM,
    },
  });

  async function onSubmit(values: z.z.infer<typeof formSchema>) {
    try {
      const deadline = Math.floor(Date.now() / 1000) + parseInt(values.deadlineDays) * 24 * 60 * 60;
      await createCampaign.mutateAsync({
        title: values.title,
        beneficiary: values.beneficiary,
        targetAmount: values.targetAmount,
        deadline,
        acceptedToken: values.acceptedToken,
      });
      toast.success("Campaign created successfully!");
      setIsOpen(false);
      form.reset();
    } catch (e: any) {
      toast.error(e.message || "Failed to create campaign");
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger asChild>
        <Button className="gap-2">
          <PlusCircle className="w-4 h-4" /> Start a Campaign
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>Create Relief Campaign</DialogTitle>
          <DialogDescription>
            Fill in the details for your relief grant. Ensure the beneficiary address is correct.
          </DialogDescription>
        </DialogHeader>
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
            <FormField
              control={form.control}
              name="title"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Campaign Title</FormLabel>
                  <FormControl>
                    <Input placeholder="Flood Relief 2024" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            <FormField
              control={form.control}
              name="beneficiary"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Beneficiary Address</FormLabel>
                  <FormControl>
                    <Input placeholder="G..." {...field} />
                  </FormControl>
                  <FormDescription>Stellar public key of the receiver.</FormDescription>
                  <FormMessage />
                </FormItem>
              )}
            />
            <div className="grid grid-cols-2 gap-4">
              <FormField
                control={form.control}
                name="targetAmount"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Target (XLM)</FormLabel>
                    <FormControl>
                      <Input type="number" placeholder="1000" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="deadlineDays"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Duration (Days)</FormLabel>
                    <FormControl>
                      <Input type="number" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>
            <Button type="submit" className="w-full" disabled={createCampaign.isPending}>
              {createCampaign.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Launch Campaign
            </Button>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
