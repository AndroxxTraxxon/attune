from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from dateutil.parser import isoparse
from typing import cast
import datetime

if TYPE_CHECKING:
  from ..models.api_response_trigger_response_data_out_schema_type_0 import ApiResponseTriggerResponseDataOutSchemaType0
  from ..models.api_response_trigger_response_data_param_schema_type_0 import ApiResponseTriggerResponseDataParamSchemaType0





T = TypeVar("T", bound="ApiResponseTriggerResponseData")



@_attrs_define
class ApiResponseTriggerResponseData:
    """ Response DTO for trigger information

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            enabled (bool): Whether the trigger is enabled Example: True.
            id (int): Trigger ID Example: 1.
            label (str): Human-readable label Example: Webhook Trigger.
            out_schema (ApiResponseTriggerResponseDataOutSchemaType0 | None): Output schema
            param_schema (ApiResponseTriggerResponseDataParamSchemaType0 | None): Parameter schema
            ref (str): Unique reference identifier Example: core.webhook.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            webhook_enabled (bool): Whether webhooks are enabled for this trigger
            description (None | str | Unset): Trigger description Example: Triggers when a webhook is received.
            pack (int | None | Unset): Pack ID (optional) Example: 1.
            pack_ref (None | str | Unset): Pack reference (optional) Example: core.
            webhook_key (None | str | Unset): Webhook key (only present if webhooks are enabled) Example:
                wh_k7j2n9p4m8q1r5w3x6z0a2b5c8d1e4f7g9h2.
     """

    created: datetime.datetime
    enabled: bool
    id: int
    label: str
    out_schema: ApiResponseTriggerResponseDataOutSchemaType0 | None
    param_schema: ApiResponseTriggerResponseDataParamSchemaType0 | None
    ref: str
    updated: datetime.datetime
    webhook_enabled: bool
    description: None | str | Unset = UNSET
    pack: int | None | Unset = UNSET
    pack_ref: None | str | Unset = UNSET
    webhook_key: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.api_response_trigger_response_data_out_schema_type_0 import ApiResponseTriggerResponseDataOutSchemaType0
        from ..models.api_response_trigger_response_data_param_schema_type_0 import ApiResponseTriggerResponseDataParamSchemaType0
        created = self.created.isoformat()

        enabled = self.enabled

        id = self.id

        label = self.label

        out_schema: dict[str, Any] | None
        if isinstance(self.out_schema, ApiResponseTriggerResponseDataOutSchemaType0):
            out_schema = self.out_schema.to_dict()
        else:
            out_schema = self.out_schema

        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, ApiResponseTriggerResponseDataParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        ref = self.ref

        updated = self.updated.isoformat()

        webhook_enabled = self.webhook_enabled

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        pack: int | None | Unset
        if isinstance(self.pack, Unset):
            pack = UNSET
        else:
            pack = self.pack

        pack_ref: None | str | Unset
        if isinstance(self.pack_ref, Unset):
            pack_ref = UNSET
        else:
            pack_ref = self.pack_ref

        webhook_key: None | str | Unset
        if isinstance(self.webhook_key, Unset):
            webhook_key = UNSET
        else:
            webhook_key = self.webhook_key


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "enabled": enabled,
            "id": id,
            "label": label,
            "out_schema": out_schema,
            "param_schema": param_schema,
            "ref": ref,
            "updated": updated,
            "webhook_enabled": webhook_enabled,
        })
        if description is not UNSET:
            field_dict["description"] = description
        if pack is not UNSET:
            field_dict["pack"] = pack
        if pack_ref is not UNSET:
            field_dict["pack_ref"] = pack_ref
        if webhook_key is not UNSET:
            field_dict["webhook_key"] = webhook_key

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.api_response_trigger_response_data_out_schema_type_0 import ApiResponseTriggerResponseDataOutSchemaType0
        from ..models.api_response_trigger_response_data_param_schema_type_0 import ApiResponseTriggerResponseDataParamSchemaType0
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        enabled = d.pop("enabled")

        id = d.pop("id")

        label = d.pop("label")

        def _parse_out_schema(data: object) -> ApiResponseTriggerResponseDataOutSchemaType0 | None:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                out_schema_type_0 = ApiResponseTriggerResponseDataOutSchemaType0.from_dict(data)



                return out_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ApiResponseTriggerResponseDataOutSchemaType0 | None, data)

        out_schema = _parse_out_schema(d.pop("out_schema"))


        def _parse_param_schema(data: object) -> ApiResponseTriggerResponseDataParamSchemaType0 | None:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = ApiResponseTriggerResponseDataParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ApiResponseTriggerResponseDataParamSchemaType0 | None, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


        ref = d.pop("ref")

        updated = isoparse(d.pop("updated"))




        webhook_enabled = d.pop("webhook_enabled")

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        def _parse_pack(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        pack = _parse_pack(d.pop("pack", UNSET))


        def _parse_pack_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        pack_ref = _parse_pack_ref(d.pop("pack_ref", UNSET))


        def _parse_webhook_key(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        webhook_key = _parse_webhook_key(d.pop("webhook_key", UNSET))


        api_response_trigger_response_data = cls(
            created=created,
            enabled=enabled,
            id=id,
            label=label,
            out_schema=out_schema,
            param_schema=param_schema,
            ref=ref,
            updated=updated,
            webhook_enabled=webhook_enabled,
            description=description,
            pack=pack,
            pack_ref=pack_ref,
            webhook_key=webhook_key,
        )


        api_response_trigger_response_data.additional_properties = d
        return api_response_trigger_response_data

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
