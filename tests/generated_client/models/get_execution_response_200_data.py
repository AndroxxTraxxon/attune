from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..models.execution_status import ExecutionStatus
from ..types import UNSET, Unset
from dateutil.parser import isoparse
from typing import cast
import datetime

if TYPE_CHECKING:
  from ..models.get_execution_response_200_data_config import GetExecutionResponse200DataConfig
  from ..models.get_execution_response_200_data_result import GetExecutionResponse200DataResult
  from ..models.get_execution_response_200_data_workflow_task_type_0 import GetExecutionResponse200DataWorkflowTaskType0





T = TypeVar("T", bound="GetExecutionResponse200Data")



@_attrs_define
class GetExecutionResponse200Data:
    """ Response DTO for execution information

        Attributes:
            action_ref (str): Action reference Example: slack.post_message.
            config (GetExecutionResponse200DataConfig): Execution configuration/parameters
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            id (int): Execution ID Example: 1.
            result (GetExecutionResponse200DataResult): Execution result/output
            status (ExecutionStatus):
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:35:00Z.
            action (int | None | Unset): Action ID (optional, may be null for ad-hoc executions) Example: 1.
            enforcement (int | None | Unset): Enforcement ID (rule enforcement that triggered this) Example: 1.
            executor (int | None | Unset): Identity ID that initiated this execution Example: 1.
            parent (int | None | Unset): Parent execution ID (for nested/child executions) Example: 1.
            started_at (datetime.datetime | None | Unset): When the execution actually started running (worker picked it
                up).
                Null if the execution hasn't started running yet. Example: 2024-01-13T10:31:00Z.
            worker (int | None | Unset): Worker ID currently assigned to this execution Example: 1.
            workflow_task (GetExecutionResponse200DataWorkflowTaskType0 | None | Unset): Workflow task metadata (only
                populated for workflow task executions)
     """

    action_ref: str
    config: GetExecutionResponse200DataConfig
    created: datetime.datetime
    id: int
    result: GetExecutionResponse200DataResult
    status: ExecutionStatus
    updated: datetime.datetime
    action: int | None | Unset = UNSET
    enforcement: int | None | Unset = UNSET
    executor: int | None | Unset = UNSET
    parent: int | None | Unset = UNSET
    started_at: datetime.datetime | None | Unset = UNSET
    worker: int | None | Unset = UNSET
    workflow_task: GetExecutionResponse200DataWorkflowTaskType0 | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.get_execution_response_200_data_config import GetExecutionResponse200DataConfig
        from ..models.get_execution_response_200_data_result import GetExecutionResponse200DataResult
        from ..models.get_execution_response_200_data_workflow_task_type_0 import GetExecutionResponse200DataWorkflowTaskType0
        action_ref = self.action_ref

        config = self.config.to_dict()

        created = self.created.isoformat()

        id = self.id

        result = self.result.to_dict()

        status = self.status.value

        updated = self.updated.isoformat()

        action: int | None | Unset
        if isinstance(self.action, Unset):
            action = UNSET
        else:
            action = self.action

        enforcement: int | None | Unset
        if isinstance(self.enforcement, Unset):
            enforcement = UNSET
        else:
            enforcement = self.enforcement

        executor: int | None | Unset
        if isinstance(self.executor, Unset):
            executor = UNSET
        else:
            executor = self.executor

        parent: int | None | Unset
        if isinstance(self.parent, Unset):
            parent = UNSET
        else:
            parent = self.parent

        started_at: None | str | Unset
        if isinstance(self.started_at, Unset):
            started_at = UNSET
        elif isinstance(self.started_at, datetime.datetime):
            started_at = self.started_at.isoformat()
        else:
            started_at = self.started_at

        worker: int | None | Unset
        if isinstance(self.worker, Unset):
            worker = UNSET
        else:
            worker = self.worker

        workflow_task: dict[str, Any] | None | Unset
        if isinstance(self.workflow_task, Unset):
            workflow_task = UNSET
        elif isinstance(self.workflow_task, GetExecutionResponse200DataWorkflowTaskType0):
            workflow_task = self.workflow_task.to_dict()
        else:
            workflow_task = self.workflow_task


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_ref": action_ref,
            "config": config,
            "created": created,
            "id": id,
            "result": result,
            "status": status,
            "updated": updated,
        })
        if action is not UNSET:
            field_dict["action"] = action
        if enforcement is not UNSET:
            field_dict["enforcement"] = enforcement
        if executor is not UNSET:
            field_dict["executor"] = executor
        if parent is not UNSET:
            field_dict["parent"] = parent
        if started_at is not UNSET:
            field_dict["started_at"] = started_at
        if worker is not UNSET:
            field_dict["worker"] = worker
        if workflow_task is not UNSET:
            field_dict["workflow_task"] = workflow_task

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.get_execution_response_200_data_config import GetExecutionResponse200DataConfig
        from ..models.get_execution_response_200_data_result import GetExecutionResponse200DataResult
        from ..models.get_execution_response_200_data_workflow_task_type_0 import GetExecutionResponse200DataWorkflowTaskType0
        d = dict(src_dict)
        action_ref = d.pop("action_ref")

        config = GetExecutionResponse200DataConfig.from_dict(d.pop("config"))




        created = isoparse(d.pop("created"))




        id = d.pop("id")

        result = GetExecutionResponse200DataResult.from_dict(d.pop("result"))




        status = ExecutionStatus(d.pop("status"))




        updated = isoparse(d.pop("updated"))




        def _parse_action(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        action = _parse_action(d.pop("action", UNSET))


        def _parse_enforcement(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        enforcement = _parse_enforcement(d.pop("enforcement", UNSET))


        def _parse_executor(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        executor = _parse_executor(d.pop("executor", UNSET))


        def _parse_parent(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        parent = _parse_parent(d.pop("parent", UNSET))


        def _parse_started_at(data: object) -> datetime.datetime | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                started_at_type_0 = isoparse(data)



                return started_at_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.datetime | None | Unset, data)

        started_at = _parse_started_at(d.pop("started_at", UNSET))


        def _parse_worker(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        worker = _parse_worker(d.pop("worker", UNSET))


        def _parse_workflow_task(data: object) -> GetExecutionResponse200DataWorkflowTaskType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                workflow_task_type_0 = GetExecutionResponse200DataWorkflowTaskType0.from_dict(data)



                return workflow_task_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(GetExecutionResponse200DataWorkflowTaskType0 | None | Unset, data)

        workflow_task = _parse_workflow_task(d.pop("workflow_task", UNSET))


        get_execution_response_200_data = cls(
            action_ref=action_ref,
            config=config,
            created=created,
            id=id,
            result=result,
            status=status,
            updated=updated,
            action=action,
            enforcement=enforcement,
            executor=executor,
            parent=parent,
            started_at=started_at,
            worker=worker,
            workflow_task=workflow_task,
        )


        get_execution_response_200_data.additional_properties = d
        return get_execution_response_200_data

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
