import { Box, Flex } from "@chakra-ui/react";
import { Routes, Route } from "react-router-dom";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import HomePage from "./pages/HomePage";
import PlaceholderPage from "./pages/PlaceholderPage";

/** 应用外壳:标题栏 + 侧边导航 + 路由内容区 */
export default function App() {
  return (
    <Flex direction="column" h="100vh">
      <TitleBar />
      <Flex flex={1} minH={0}>
        <Sidebar />
        <Box flex={1} overflowY="auto" px={8} py={7}>
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/download" element={<PlaceholderPage kind="download" />} />
            <Route path="/accounts" element={<PlaceholderPage kind="accounts" />} />
            <Route path="/settings" element={<PlaceholderPage kind="settings" />} />
          </Routes>
        </Box>
      </Flex>
    </Flex>
  );
}