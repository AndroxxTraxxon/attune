from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..models.work_queue_batch_mode import WorkQueueBatchMode
from ..models.work_queue_update_strategy import WorkQueueUpdateStrategy
from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.create_work_queue_request_action_params import CreateWorkQueueRequestActionParams
  from ..models.create_work_queue_request_config import CreateWorkQueueRequestConfig
  from ..models.create_work_queue_request_item_schema import CreateWorkQueueRequestItemSchema





T = TypeVar("T", bound="CreateWorkQueueRequest")



@_attrs_define
class CreateWorkQueueRequest:
    """ 
        Attributes:
            dispatch_action_ref (str):  Example: core.process_item.
            label (str):  Example: Core Inbox.
            ref (str):  Example: core.inbox.
            accepting_new_items (bool | Unset):  Default: True. Example: True.
            action_params (CreateWorkQueueRequestActionParams | Unset):
            allow_pending_update (bool | Unset):  Default: False.
            batch_mode (WorkQueueBatchMode | Unset):
            config (CreateWorkQueueRequestConfig | Unset):
            default_priority (int | Unset):  Default: 0.
            description (None | str | Unset):  Example: Dispatches inbound work items to the core processor.
            enabled (bool | Unset):  Default: True. Example: True.
            item_schema (CreateWorkQueueRequestItemSchema | Unset):
            pack_ref (None | str | Unset):  Example: core.
            update_strategy (WorkQueueUpdateStrategy | Unset):
     """

    dispatch_action_ref: str
    label: str
    ref: str
    accepting_new_items: bool | Unset = True
    action_params: CreateWorkQueueRequestActionParams | Unset = UNSET
    allow_pending_update: bool | Unset = False
    batch_mode: WorkQueueBatchMode | Unset = UNSET
    config: CreateWorkQueueRequestConfig | Unset = UNSET
    default_priority: int | Unset = 0
    description: None | str | Unset = UNSET
    enabled: bool | Unset = True
    item_schema: CreateWorkQueueRequestItemSchema | Unset = UNSET
    pack_ref: None | str | Unset = UNSET
    update_strategy: WorkQueueUpdateStrategy | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.create_work_queue_request_action_params import CreateWorkQueueRequestActionParams
        from ..models.create_work_queue_request_config import CreateWorkQueueRequestConfig
        from ..models.create_work_queue_request_item_schema import CreateWorkQueueRequestItemSchema
        dispatch_action_ref = self.dispatch_action_ref

        label = self.label

        ref = self.ref

        accepting_new_items = self.accepting_new_items

        action_params: dict[str, Any] | Unset = UNSET
        if not isinstance(self.action_params, Unset):
            action_params = self.action_params.to_dict()

        allow_pending_update = self.allow_pending_update

        batch_mode: str | Unset = UNSET
        if not isinstance(self.batch_mode, Unset):
            batch_mode = self.batch_mode.value


        config: dict[str, Any] | Unset = UNSET
        if not isinstance(self.config, Unset):
            config = self.config.to_dict()

        default_priority = self.default_priority

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        enabled = self.enabled

        item_schema: dict[str, Any] | Unset = UNSET
        if not isinstance(self.item_schema, Unset):
            item_schema = self.item_schema.to_dict()

        pack_ref: None | str | Unset
        if isinstance(self.pack_ref, Unset):
            pack_ref = UNSET
        else:
            pack_ref = self.pack_ref

        update_strategy: str | Unset = UNSET
        if not isinstance(self.update_strategy, Unset):
            update_strategy = self.update_strategy.value



        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "dispatch_action_ref": dispatch_action_ref,
            "label": label,
            "ref": ref,
        })
        if accepting_new_items is not UNSET:
            field_dict["accepting_new_items"] = accepting_new_items
        if action_params is not UNSET:
            field_dict["action_params"] = action_params
        if allow_pending_update is not UNSET:
            field_dict["allow_pending_update"] = allow_pending_update
        if batch_mode is not UNSET:
            field_dict["batch_mode"] = batch_mode
        if config is not UNSET:
            field_dict["config"] = config
        if default_priority is not UNSET:
            field_dict["default_priority"] = default_priority
        if description is not UNSET:
            field_dict["description"] = description
        if enabled is not UNSET:
            field_dict["enabled"] = enabled
        if item_schema is not UNSET:
            field_dict["item_schema"] = item_schema
        if pack_ref is not UNSET:
            field_dict["pack_ref"] = pack_ref
        if update_strategy is not UNSET:
            field_dict["update_strategy"] = update_strategy

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.create_work_queue_request_action_params import CreateWorkQueueRequestActionParams
        from ..models.create_work_queue_request_config import CreateWorkQueueRequestConfig
        from ..models.create_work_queue_request_item_schema import CreateWorkQueueRequestItemSchema
        d = dict(src_dict)
        dispatch_action_ref = d.pop("dispatch_action_ref")

        label = d.pop("label")

        ref = d.pop("ref")

        accepting_new_items = d.pop("accepting_new_items", UNSET)

        _action_params = d.pop("action_params", UNSET)
        action_params: CreateWorkQueueRequestActionParams | Unset
        if isinstance(_action_params,  Unset):
            action_params = UNSET
        else:
            action_params = CreateWorkQueueRequestActionParams.from_dict(_action_params)




        allow_pending_update = d.pop("allow_pending_update", UNSET)

        _batch_mode = d.pop("batch_mode", UNSET)
        batch_mode: WorkQueueBatchMode | Unset
        if isinstance(_batch_mode,  Unset):
            batch_mode = UNSET
        else:
            batch_mode = WorkQueueBatchMode(_batch_mode)




        _config = d.pop("config", UNSET)
        config: CreateWorkQueueRequestConfig | Unset
        if isinstance(_config,  Unset):
            config = UNSET
        else:
            config = CreateWorkQueueRequestConfig.from_dict(_config)




        default_priority = d.pop("default_priority", UNSET)

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        enabled = d.pop("enabled", UNSET)

        _item_schema = d.pop("item_schema", UNSET)
        item_schema: CreateWorkQueueRequestItemSchema | Unset
        if isinstance(_item_schema,  Unset):
            item_schema = UNSET
        else:
            item_schema = CreateWorkQueueRequestItemSchema.from_dict(_item_schema)




        def _parse_pack_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        pack_ref = _parse_pack_ref(d.pop("pack_ref", UNSET))


        _update_strategy = d.pop("update_strategy", UNSET)
        update_strategy: WorkQueueUpdateStrategy | Unset
        if isinstance(_update_strategy,  Unset):
            update_strategy = UNSET
        else:
            update_strategy = WorkQueueUpdateStrategy(_update_strategy)




        create_work_queue_request = cls(
            dispatch_action_ref=dispatch_action_ref,
            label=label,
            ref=ref,
            accepting_new_items=accepting_new_items,
            action_params=action_params,
            allow_pending_update=allow_pending_update,
            batch_mode=batch_mode,
            config=config,
            default_priority=default_priority,
            description=description,
            enabled=enabled,
            item_schema=item_schema,
            pack_ref=pack_ref,
            update_strategy=update_strategy,
        )


        create_work_queue_request.additional_properties = d
        return create_work_queue_request

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
