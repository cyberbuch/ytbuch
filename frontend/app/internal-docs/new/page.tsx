"use client";

import { InternalDocForm } from "../../../components/InternalDocForm";
import { RequireAuth } from "../../../components/RequireAuth";
import { useI18n } from "../../../components/I18nProvider";

function Inner() {
  const { t } = useI18n();
  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("idoc.new")}</h1>
      </div>
      <InternalDocForm mode="create" />
    </div>
  );
}

export default function NewInternalDocPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
