import { useState } from "react";
import { Pencil, Plus, X, LogIn, LogOut } from "lucide-react";
import SchemaBuilder from "@/components/common/SchemaBuilder";
import type { ParamDefinition } from "@/types/workflow";

interface WorkflowInputsPanelProps {
  parameters: Record<string, ParamDefinition>;
  output: Record<string, ParamDefinition>;
  onParametersChange: (parameters: Record<string, ParamDefinition>) => void;
  onOutputChange: (output: Record<string, ParamDefinition>) => void;
}

type ModalTarget = "parameters" | "output" | null;

function ParamSummaryList({
  schema,
  emptyMessage,
  onEdit,
}: {
  schema: Record<string, ParamDefinition>;
  emptyMessage: string;
  onEdit: () => void;
}) {
  const entries = Object.entries(schema);

  if (entries.length === 0) {
    return (
      <button
        onClick={onEdit}
        className="w-full px-3 py-4 border-2 border-dashed border-gray-200 rounded-lg text-center hover:border-blue-300 hover:bg-blue-50/30 transition-colors group"
      >
        <Plus className="w-4 h-4 text-gray-300 group-hover:text-blue-400 mx-auto mb-1" />
        <span className="text-[11px] text-gray-400 group-hover:text-blue-500">
          {emptyMessage}
        </span>
      </button>
    );
  }

  return (
    <div className="space-y-1">
      {entries.map(([name, def]) => (
        <div
          key={name}
          className="flex items-center gap-1.5 px-2 py-1.5 bg-white border border-gray-150 rounded-md"
        >
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-1.5">
              <span className="font-mono text-[11px] font-medium text-gray-800 truncate">
                {name}
              </span>
              {def.required && (
                <span className="text-[9px] font-semibold text-red-500">*</span>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-[10px] text-blue-600/70">{def.type}</span>
              {def.secret && (
                <span className="text-[9px] text-amber-500">secret</span>
              )}
              {def.default !== undefined && (
                <span
                  className="text-[9px] text-gray-400 truncate max-w-[80px]"
                  title={`default: ${JSON.stringify(def.default)}`}
                >
                  = {JSON.stringify(def.default)}
                </span>
              )}
            </div>
          </div>
        </div>
      ))}
      <button
        onClick={onEdit}
        className="flex items-center gap-1 px-2 py-1 text-[11px] text-gray-400 hover:text-blue-600 transition-colors w-full"
      >
        <Pencil className="w-3 h-3" />
        Edit
      </button>
    </div>
  );
}

export default function WorkflowInputsPanel({
  parameters,
  output,
  onParametersChange,
  onOutputChange,
}: WorkflowInputsPanelProps) {
  const [modalTarget, setModalTarget] = useState<ModalTarget>(null);

  // Draft state for the modal so changes only apply on confirm
  const [draftSchema, setDraftSchema] = useState<
    Record<string, ParamDefinition>
  >({});

  const openModal = (target: ModalTarget) => {
    if (target === "parameters") {
      setDraftSchema({ ...parameters });
    } else if (target === "output") {
      setDraftSchema({ ...output });
    }
    setModalTarget(target);
  };

  const handleConfirm = () => {
    if (modalTarget === "parameters") {
      onParametersChange(draftSchema);
    } else if (modalTarget === "output") {
      onOutputChange(draftSchema);
    }
    setModalTarget(null);
  };

  const handleCancel = () => {
    setModalTarget(null);
  };

  const modalLabel =
    modalTarget === "parameters" ? "Input Parameters" : "Output";

  return (
    <>
      <div className="flex flex-col h-full overflow-hidden">
        <div className="flex-1 overflow-y-auto p-3 space-y-4">
          {/* Input Parameters */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <div className="flex items-center gap-1.5">
                <LogIn className="w-3.5 h-3.5 text-green-500" />
                <h4 className="text-xs font-semibold text-gray-600 uppercase tracking-wider">
                  Inputs
                </h4>
              </div>
              {Object.keys(parameters).length > 0 && (
                <span className="text-[10px] text-gray-400">
                  {Object.keys(parameters).length}
                </span>
              )}
            </div>
            <p className="text-[10px] text-gray-400 mb-2">
              Referenced via{" "}
              <code className="px-0.5 bg-gray-100 rounded">
                {"{{ params.<name> }}"}
              </code>
            </p>
            <ParamSummaryList
              schema={parameters}
              emptyMessage="Add input parameters"
              onEdit={() => openModal("parameters")}
            />
          </div>

          {/* Output Schema */}
          <div className="border-t border-gray-200 pt-3">
            <div className="flex items-center justify-between mb-1.5">
              <div className="flex items-center gap-1.5">
                <LogOut className="w-3.5 h-3.5 text-violet-500" />
                <h4 className="text-xs font-semibold text-gray-600 uppercase tracking-wider">
                  Output
                </h4>
              </div>
              {Object.keys(output).length > 0 && (
                <span className="text-[10px] text-gray-400">
                  {Object.keys(output).length}
                </span>
              )}
            </div>
            <p className="text-[10px] text-gray-400 mb-2">
              Values this workflow produces on completion.
            </p>
            <ParamSummaryList
              schema={output}
              emptyMessage="Add output schema"
              onEdit={() => openModal("output")}
            />
          </div>
        </div>
      </div>

      {/* Full-screen modal for SchemaBuilder editing */}
      {modalTarget && (
        <div className="fixed inset-0 z-[70] flex items-center justify-center">
          {/* Backdrop */}
          <div
            className="absolute inset-0 bg-black/40"
            onClick={handleCancel}
          />
          {/* Modal */}
          <div className="relative bg-white rounded-xl shadow-2xl border border-gray-200 flex flex-col w-full max-w-2xl max-h-[80vh] mx-4">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 flex-shrink-0">
              <div>
                <h2 className="text-base font-semibold text-gray-900">
                  {modalLabel}
                </h2>
                <p className="text-xs text-gray-500 mt-0.5">
                  {modalTarget === "parameters"
                    ? "Define the inputs this workflow accepts when executed."
                    : "Define the outputs this workflow produces upon completion."}
                </p>
              </div>
              <button
                onClick={handleCancel}
                className="p-1.5 rounded-lg hover:bg-gray-100 text-gray-400 hover:text-gray-600 transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Body */}
            <div className="flex-1 overflow-y-auto px-6 py-4">
              <SchemaBuilder
                value={draftSchema}
                onChange={(schema) =>
                  setDraftSchema(
                    schema as unknown as Record<string, ParamDefinition>,
                  )
                }
                placeholder={
                  modalTarget === "parameters"
                    ? '{"message": {"type": "string", "required": true}}'
                    : '{"result": {"type": "string"}}'
                }
              />
            </div>

            {/* Footer */}
            <div className="flex items-center justify-end gap-2 px-6 py-3 border-t border-gray-200 bg-gray-50 rounded-b-xl flex-shrink-0">
              <button
                onClick={handleCancel}
                className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleConfirm}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors shadow-sm"
              >
                Apply
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
