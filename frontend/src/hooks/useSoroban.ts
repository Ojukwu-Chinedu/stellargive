"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getCampaign, getRecentCampaigns, submitTransaction, CONTRACT_ID, toStroops, getEvents } from "@/lib/soroban";
import { Address, nativeToScVal } from "@stellar/stellar-sdk";
import { useWallet } from "@/lib/WalletProvider";

export function useCampaign(id: bigint) {
  return useQuery({
    queryKey: ["campaign", id.toString()],
    queryFn: () => getCampaign(id),
  });
}

export function useRecentCampaigns() {
  return useQuery({
    queryKey: ["campaigns", "recent"],
    queryFn: () => getRecentCampaigns(),
  });
}

export function useCreateCampaign() {
  const { address } = useWallet();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      beneficiary: string;
      title: string;
      targetAmount: string;
      deadline: number;
      acceptedToken: string;
    }) => {
      if (!address) throw new Error("Wallet not connected");

      const args = [
        new Address(address).toScVal(),
        new Address(params.beneficiary).toScVal(),
        nativeToScVal(params.title, { type: "string" }),
        nativeToScVal(toStroops(params.targetAmount), { type: "i128" }),
        nativeToScVal(BigInt(params.deadline), { type: "u64" }),
        new Address(params.acceptedToken).toScVal(),
      ];

      return submitTransaction(address, "create_campaign", args);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["campaigns"] });
    },
  });
}

export function useDonate() {
  const { address } = useWallet();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: { campaignId: bigint; amount: string }) => {
      if (!address) throw new Error("Wallet not connected");

      const args = [
        new Address(address).toScVal(),
        nativeToScVal(params.campaignId, { type: "u64" }),
        nativeToScVal(toStroops(params.amount), { type: "i128" }),
      ];

      return submitTransaction(address, "donate", args);
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["campaign", variables.campaignId.toString()] });
      queryClient.invalidateQueries({ queryKey: ["campaigns"] });
    },
  });
}

export function useClaimFunds() {
  const { address } = useWallet();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (campaignId: bigint) => {
      if (!address) throw new Error("Wallet not connected");

      const args = [
        new Address(address).toScVal(),
        nativeToScVal(campaignId, { type: "u64" }),
      ];

      return submitTransaction(address, "claim_funds", args);
    },
    onSuccess: (_, campaignId) => {
      queryClient.invalidateQueries({ queryKey: ["campaign", campaignId.toString()] });
      queryClient.invalidateQueries({ queryKey: ["campaigns"] });
    },
  });
}

export function useEvents() {
  return useQuery({
    queryKey: ["events"],
    queryFn: () => getEvents(),
    refetchInterval: 5000,
  });
}
