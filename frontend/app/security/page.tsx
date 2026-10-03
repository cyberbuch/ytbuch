"use client";

// Старият адрес /security отваря таба за 2FA в профила.

import { useEffect } from "react";
import { useRouter } from "next/navigation";

export default function SecurityRedirect() {
  const router = useRouter();
  useEffect(() => {
    router.replace("/profile?tab=security");
  }, [router]);
  return null;
}
