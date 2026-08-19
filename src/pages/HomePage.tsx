import {
  Alert,
  AlertIcon,
  Badge,
  Box,
  Button,
  Flex,
  Skeleton,
  Text,
  Tooltip,
  VStack,
} from "@chakra-ui/react";
import { DownloadIcon, RepeatIcon } from "@chakra-ui/icons";
import type { ManifestVersion, LatestVersions } from "../types/version";
import { useVersionManifest } from "../hooks/useVersionManifest";
import { useVersionDownload } from "../hooks/useVersionDownload";
import { useVersionJar } from "../hooks/useVersionJar";
import { useVersionAssets } from "../hooks/useVersionAssets";

/** 把版本按 正式版/快照版 分组,并按发布时间倒序 */
function groupVersions(versions: ManifestVersion[]) {
  const release = versions
    .filter((v) => v.type === "release")
    .sort((a, b) => b.releaseTime.localeCompare(a.releaseTime));
  const snapshot = versions
    .filter((v) => v.type === "snapshot")
    .sort((a, b) => b.releaseTime.localeCompare(a.releaseTime));
  return { release, snapshot };
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

/** 单个版本卡片 */
function VersionCard({
  version,
  isLatest,
}: {
  version: ManifestVersion;
  isLatest: boolean;
}) {
  const isRelease = version.type === "release";
  const { state, download } = useVersionDownload(version.id, version.url);
  const jar = useVersionJar(version.id);
  const assets = useVersionAssets(version.id);
  return (
    <Flex
      align="center"
      gap={4}
      bg="white"
      borderRadius="card"
      border="1px solid"
      borderColor="brand.100"
      boxShadow="card"
      px={4}
      py={3.5}
      _hover={{ boxShadow: "cardHover", transform: "translateY(-1px)" }}
      transition="all 0.15s"
    >
      {/* MC 像素块图标 */}
      <Box
        w={9}
        h={9}
        flexShrink={0}
        borderRadius="lg"
        bg={isRelease ? "grass.100" : "brand.100"}
        display="flex"
        alignItems="center"
        justifyContent="center"
      >
        <Box
          w="14px"
          h="14px"
          borderRadius="3px"
          bg={isRelease ? "grass.500" : "brand.400"}
          boxShadow="0 0 0 3px rgba(255,255,255,0.6)"
        />
      </Box>

      <Box flex={1} minW={0}>
        <Flex align="center" gap={2}>
          <Text fontWeight="800" fontSize="md" color="gray.800" noOfLines={1}>
            {version.id}
          </Text>
          <Badge colorScheme={isRelease ? "grass" : "brand"}>
            {isRelease ? "正式版" : "快照版"}
          </Badge>
          {isLatest && (
            <Badge colorScheme="yellow" variant="solid">
              最新
            </Badge>
          )}
        </Flex>
        <Text fontSize="xs" color="gray.400" mt={0.5}>
          发布于 {formatDate(version.releaseTime)}
        </Text>
      </Box>

      {state.status === "done" ? (
        <Tooltip label={`已保存: ${state.path}`} placement="top">
          <Button
            size="sm"
            leftIcon={<DownloadIcon />}
            onClick={download}
            colorScheme="grass"
            variant="outline"
          >
            已下载
          </Button>
        </Tooltip>
      ) : (
        <Button
          size="sm"
          leftIcon={<DownloadIcon />}
          onClick={download}
          isLoading={state.status === "downloading"}
        >
          {state.status === "idle" && "下载"}
          {state.status === "downloading" && "下载中"}
          {state.status === "error" && "重试"}
        </Button>
      )}
      {jar.state.status === "done" ? (
        <Tooltip label={`已保存: ${jar.state.path}`} placement="top">
          <Button
            size="sm"
            colorScheme="grass"
            variant="outline"
            onClick={jar.download}
          >
            客户端
          </Button>
        </Tooltip>
      ) : (
        <Tooltip
          label={jar.state.status === "error" ? jar.state.message : "下载客户端 jar"}
          placement="top"
        >
          <Button
            size="sm"
            onClick={jar.download}
            isLoading={jar.state.status === "downloading"}
          >
            {jar.state.status === "idle" && "客户端"}
            {jar.state.status === "downloading" && "下载中"}
            {jar.state.status === "error" && "重试"}
          </Button>
        </Tooltip>
      )}
      {assets.state.status === "done" ? (
        <Tooltip
          label={`资源完成: 新增 ${assets.state.summary.downloaded} / 共 ${assets.state.summary.total}, 跳过 ${assets.state.summary.skipped}`}
          placement="top"
        >
          <Button
            size="sm"
            colorScheme="grass"
            variant="outline"
            onClick={assets.download}
          >
            资源
          </Button>
        </Tooltip>
      ) : (
        <Tooltip
          label={
            assets.state.status === "error"
              ? assets.state.message
              : "下载全部资源(音效/语言/字体等, 首次较大)"
          }
          placement="top"
        >
          <Button
            size="sm"
            onClick={assets.download}
            isLoading={assets.state.status === "downloading"}
          >
            {assets.state.status === "idle" && "资源"}
            {assets.state.status === "downloading" && "下载中"}
            {assets.state.status === "error" && "重试"}
          </Button>
        </Tooltip>
      )}
      <Tooltip label="启动功能将在后续里程碑实现" placement="top">
        <Box as="span">
          <Button size="sm" isDisabled>
            启动
          </Button>
        </Box>
      </Tooltip>
    </Flex>
  );
}

/** 版本分组区块 */
function VersionGroup({
  title,
  count,
  versions,
  latestId,
}: {
  title: string;
  count: number;
  versions: ManifestVersion[];
  latestId?: string;
}) {
  if (versions.length === 0) return null;
  return (
    <Box mb={8}>
      <Text fontWeight="800" fontSize="lg" color="gray.700" mb={3}>
        {title}
        <Box as="span" color="gray.400" fontSize="sm" fontWeight="600" ml={2}>
          {count} 个版本
        </Box>
      </Text>
      <VStack spacing={2.5} align="stretch">
        {versions.map((v) => (
          <VersionCard key={v.id} version={v} isLatest={v.id === latestId} />
        ))}
      </VStack>
    </Box>
  );
}

/** 加载中的骨架卡片 */
function LoadingSkeleton() {
  return (
    <VStack spacing={2.5} align="stretch">
      {Array.from({ length: 5 }, (_, i) => (
        <Flex key={i} align="center" gap={4} bg="white" borderRadius="card" p={4}>
          <Skeleton w={9} h={9} borderRadius="lg" />
          <Box flex={1}>
            <Skeleton h="16px" w="120px" mb={2} />
            <Skeleton h="12px" w="80px" />
          </Box>
          <Skeleton w="64px" h="32px" borderRadius="full" />
        </Flex>
      ))}
    </VStack>
  );
}

/** 主页:Minecraft 版本列表 */
export default function HomePage() {
  const result = useVersionManifest();
  const reload = result.reload;

  return (
    <Box maxW="880px" mx="auto">
      <Flex direction="column" gap={3} mb={7}>
        {/* MC 草方块像素条 */}
        <Flex gap="3px">
          {Array.from({ length: 22 }, (_, i) => (
            <Box key={i} w="7px" h="7px" borderRadius="2px" bg={i % 2 === 0 ? "grass.400" : "grass.600"} />
          ))}
        </Flex>

        <Flex align="baseline" justify="space-between">
          <Box>
            <Text fontSize="3xl" fontWeight="800" color="gray.800">
              欢迎回来
            </Text>
            <Text fontSize="sm" color="gray.500" mt={1}>
              从 Mojang 官方清单读取版本 · 选择一个版本开始冒险
            </Text>
          </Box>
          <Button
            size="sm"
            variant="ghost"
            leftIcon={<RepeatIcon />}
            onClick={reload}
            isLoading={result.status === "loading"}
          >
            刷新
          </Button>
        </Flex>
      </Flex>

      {result.status === "loading" && <LoadingSkeleton />}

      {result.status === "error" && (
        <Alert status="error" borderRadius="card" bg="red.50" flexDirection="column" gap={3} py={5}>
          <AlertIcon />
          <Box textAlign="center">
            <Text fontWeight="700" color="red.600">
              版本清单获取失败
            </Text>
            <Text fontSize="sm" color="red.400" mt={1} noOfLines={3}>
              {result.message}
            </Text>
          </Box>
          <Button size="sm" colorScheme="red" variant="outline" onClick={reload}>
            重试
          </Button>
        </Alert>
      )}

      {result.status === "success" && result.manifest && (
        <VersionGroups latest={result.manifest.latest} versions={result.manifest.versions} />
      )}
    </Box>
  );
}

function VersionGroups({
  latest,
  versions,
}: {
  latest: LatestVersions;
  versions: ManifestVersion[];
}) {
  const { release, snapshot } = groupVersions(versions);
  const isEmpty = release.length === 0 && snapshot.length === 0;

  if (isEmpty) {
    return (
      <Alert status="info" borderRadius="card">
        <AlertIcon />
        暂时没有版本数据
      </Alert>
    );
  }

  return (
    <>
      <VersionGroup title="正式版" count={release.length} versions={release} latestId={latest.release} />
      <VersionGroup title="快照版" count={snapshot.length} versions={snapshot} latestId={latest.snapshot} />
    </>
  );
}