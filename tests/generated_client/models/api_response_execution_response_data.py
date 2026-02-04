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
  from ..models.api_response_execution_response_data_config import ApiResponseExecutionResponseDataConfig
  from ..models.api_response_execution_response_data_result import ApiResponseExecutionResponseDataResult





T = TypeVar("T", bound="ApiResponseExecutionResponseData")



@_attrs_define
class ApiResponseExecutionResponseData:
    """ Response DTO for execution information

        Attributes:
            action_ref (str): Action reference Example: slack.post_message.
            config (ApiResponseExecutionResponseDataConfig): Execution configuration/parameters
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            id (int): Execution ID Example: 1.
            result (ApiResponseExecutionResponseDataResult): Execution result/output
            status (ExecutionStatus):
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:35:00Z.
            action (int | None | Unset): Action ID (optional, may be null for ad-hoc executions) Example: 1.
            enforcement (int | None | Unset): Enforcement ID (rule enforcement that triggered this) Example: 1.
            executor (int | None | Unset): Executor ID (worker/executor that ran this) Example: 1.
            parent (int | None | Unset): Parent execution ID (for nested/child executions) Example: 1.
     """

    action_ref: str
    config: ApiResponseExecutionResponseDataConfig
    created: datetime.datetime
    id: int
    result: ApiResponseExecutionResponseDataResult
    status: ExecutionStatus
    updated: datetime.datetime
    action: int | None | Unset = UNSET
    enforcement: int | None | Unset = UNSET
    executor: int | None | Unset = UNSET
    parent: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.api_response_execution_response_data_config import ApiResponseExecutionResponseDataConfig
        from ..models.api_response_execution_response_data_result import ApiResponseExecutionResponseDataResult
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

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.api_response_execution_response_data_config import ApiResponseExecutionResponseDataConfig
        from ..models.api_response_execution_response_data_result import ApiResponseExecutionResponseDataResult
        d = dict(src_dict)
        action_ref = d.pop("action_ref")

        config = ApiResponseExecutionResponseDataConfig.from_dict(d.pop("config"))




        created = isoparse(d.pop("created"))




        id = d.pop("id")

        result = ApiResponseExecutionResponseDataResult.from_dict(d.pop("result"))




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


        api_response_execution_response_data = cls(
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
        )


        api_response_execution_response_data.additional_properties = d
        return api_response_execution_response_data

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
