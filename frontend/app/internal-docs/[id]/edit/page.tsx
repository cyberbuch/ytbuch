"use client";

import { useParams } from "next/navigation";
import { InternalDocForm } from "../../../../components/InternalDocForm";
import { RequireAuth } from "../../../../components/RequireAuth";
import { useI18n } from "../../../../components/I18nProvider";

function Inner() {
  const { t } = useI18n();
  const params = useParams();
  const id = Number(params.id);
  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("idoc.edit")}</h1>
      </div>
      <InternalDocForm mode="edit" docId={id} />
    </div>
  );
}

export default function EditInternalDocPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
