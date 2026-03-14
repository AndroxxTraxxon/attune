{{- define "attune.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "attune.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name (include "attune.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "attune.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" -}}
{{- end -}}

{{- define "attune.labels" -}}
helm.sh/chart: {{ include "attune.chart" . }}
app.kubernetes.io/name: {{ include "attune.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "attune.selectorLabels" -}}
app.kubernetes.io/name: {{ include "attune.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "attune.componentLabels" -}}
{{ include "attune.selectorLabels" .root }}
app.kubernetes.io/component: {{ .component }}
{{- end -}}

{{- define "attune.image" -}}
{{- $root := .root -}}
{{- $image := .image -}}
{{- $registry := $root.Values.global.imageRegistry -}}
{{- $namespace := $root.Values.global.imageNamespace -}}
{{- $repository := $image.repository -}}
{{- $tag := default $root.Values.global.imageTag $image.tag -}}
{{- if and $registry $namespace -}}
{{- printf "%s/%s/%s:%s" $registry $namespace $repository $tag -}}
{{- else if $registry -}}
{{- printf "%s/%s:%s" $registry $repository $tag -}}
{{- else -}}
{{- printf "%s:%s" $repository $tag -}}
{{- end -}}
{{- end -}}

{{- define "attune.secretName" -}}
{{- if .Values.security.existingSecret -}}
{{- .Values.security.existingSecret -}}
{{- else -}}
{{- printf "%s-secrets" (include "attune.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "attune.postgresqlServiceName" -}}
{{- if .Values.database.host -}}
{{- .Values.database.host -}}
{{- else -}}
{{- printf "%s-postgresql" (include "attune.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "attune.rabbitmqServiceName" -}}
{{- if .Values.rabbitmq.host -}}
{{- .Values.rabbitmq.host -}}
{{- else -}}
{{- printf "%s-rabbitmq" (include "attune.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "attune.redisServiceName" -}}
{{- if .Values.redis.host -}}
{{- .Values.redis.host -}}
{{- else -}}
{{- printf "%s-redis" (include "attune.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "attune.databaseUrl" -}}
{{- if .Values.database.url -}}
{{- .Values.database.url -}}
{{- else -}}
{{- printf "postgresql://%s:%s@%s:%v/%s" .Values.database.username .Values.database.password (include "attune.postgresqlServiceName" .) .Values.database.port .Values.database.database -}}
{{- end -}}
{{- end -}}

{{- define "attune.rabbitmqUrl" -}}
{{- if .Values.rabbitmq.url -}}
{{- .Values.rabbitmq.url -}}
{{- else -}}
{{- printf "amqp://%s:%s@%s:%v" .Values.rabbitmq.username .Values.rabbitmq.password (include "attune.rabbitmqServiceName" .) .Values.rabbitmq.port -}}
{{- end -}}
{{- end -}}

{{- define "attune.redisUrl" -}}
{{- if .Values.redis.url -}}
{{- .Values.redis.url -}}
{{- else -}}
{{- printf "redis://%s:%v" (include "attune.redisServiceName" .) .Values.redis.port -}}
{{- end -}}
{{- end -}}

{{- define "attune.apiServiceName" -}}
{{- printf "%s-api" (include "attune.fullname" .) -}}
{{- end -}}

{{- define "attune.notifierServiceName" -}}
{{- printf "%s-notifier" (include "attune.fullname" .) -}}
{{- end -}}
