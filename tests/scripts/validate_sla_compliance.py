#!/usr/bin/env python3
"""
PRISM Performance SLA Validation Script

Validates test results against defined SLA requirements for CI/CD pipeline integration.
Used by GitHub Actions to enforce performance quality gates.
"""

import json
import argparse
import sys
import logging
from typing import Dict, Any, List, Optional
from dataclasses import dataclass


@dataclass
class SLAThreshold:
    """Performance SLA threshold definition"""
    metric_name: str
    threshold_value: float
    comparison_type: str  # 'max', 'min', 'exact'
    unit: str


@dataclass
class SLAResult:
    """SLA validation result"""
    metric_name: str
    measured_value: float
    threshold_value: float
    passed: bool
    deviation_percent: float
    unit: str


class SLAValidator:
    """Performance SLA validation engine"""
    
    def __init__(self):
        self.logger = logging.getLogger(__name__)
        
    def parse_test_results(self, results_file: str) -> Dict[str, Any]:
        """Parse JSON test results file"""
        try:
            with open(results_file, 'r') as f:
                data = json.load(f)
            return data
        except FileNotFoundError:
            self.logger.error(f"Test results file not found: {results_file}")
            sys.exit(1)
        except json.JSONDecodeError as e:
            self.logger.error(f"Invalid JSON in results file: {e}")
            sys.exit(1)
            
    def extract_performance_metrics(self, test_data: Dict[str, Any]) -> Dict[str, float]:
        """Extract performance metrics from test results"""
        metrics = {}
        
        # Extract metrics from different test result formats
        if 'performance_metrics' in test_data:
            metrics.update(test_data['performance_metrics'])
            
        if 'benchmarks' in test_data:
            for benchmark in test_data['benchmarks']:
                if 'metric' in benchmark and 'value' in benchmark:
                    metrics[benchmark['metric']] = float(benchmark['value'])
                    
        # Look for specific metric patterns in test output
        if 'test_results' in test_data:
            for test in test_data['test_results']:
                if 'stdout' in test:
                    metrics.update(self._parse_stdout_metrics(test['stdout']))
                    
        return metrics
        
    def _parse_stdout_metrics(self, stdout: str) -> Dict[str, float]:
        """Parse performance metrics from test stdout"""
        metrics = {}
        
        # Common metric patterns
        patterns = [
            (r'storage_throughput_mbs:\s*(\d+\.?\d*)', 'storage_throughput_mbs'),
            (r'network_latency_ms:\s*(\d+\.?\d*)', 'network_latency_ms'),
            (r'consensus_latency_ms:\s*(\d+\.?\d*)', 'consensus_latency_ms'),
            (r'api_response_ms:\s*(\d+\.?\d*)', 'api_response_ms'),
            (r'memory_usage_mb:\s*(\d+\.?\d*)', 'memory_usage_mb'),
            (r'deduplication_ratio:\s*(\d+\.?\d*)', 'deduplication_ratio'),
            (r'compression_ratio:\s*(\d+\.?\d*)', 'compression_ratio')
        ]
        
        import re
        for pattern, metric_name in patterns:
            matches = re.findall(pattern, stdout, re.IGNORECASE)
            if matches:
                metrics[metric_name] = float(matches[-1])  # Take last match
                
        return metrics
        
    def validate_sla(self, metrics: Dict[str, float], thresholds: List[SLAThreshold]) -> List[SLAResult]:
        """Validate metrics against SLA thresholds"""
        results = []
        
        for threshold in thresholds:
            if threshold.metric_name not in metrics:
                self.logger.warning(f"Metric {threshold.metric_name} not found in test results")
                continue
                
            measured_value = metrics[threshold.metric_name]
            threshold_value = threshold.threshold_value
            
            # Determine if SLA is met
            if threshold.comparison_type == 'max':
                passed = measured_value <= threshold_value
                deviation = ((measured_value - threshold_value) / threshold_value) * 100
            elif threshold.comparison_type == 'min':
                passed = measured_value >= threshold_value
                deviation = ((threshold_value - measured_value) / threshold_value) * 100
            elif threshold.comparison_type == 'exact':
                passed = abs(measured_value - threshold_value) < 0.01
                deviation = ((abs(measured_value - threshold_value)) / threshold_value) * 100
            else:
                self.logger.error(f"Unknown comparison type: {threshold.comparison_type}")
                continue
                
            results.append(SLAResult(
                metric_name=threshold.metric_name,
                measured_value=measured_value,
                threshold_value=threshold_value,
                passed=passed,
                deviation_percent=deviation if not passed else 0.0,
                unit=threshold.unit
            ))
            
        return results
        
    def generate_report(self, results: List[SLAResult]) -> str:
        """Generate SLA validation report"""
        report_lines = [
            "# PRISM Performance SLA Validation Report",
            "",
            "| Metric | Measured | Threshold | Status | Deviation | Unit |",
            "|--------|----------|-----------|--------|-----------|------|"
        ]
        
        all_passed = True
        critical_failures = []
        
        for result in results:
            status = "✅ PASS" if result.passed else "❌ FAIL"
            if not result.passed:
                all_passed = False
                if result.deviation_percent > 20:  # Critical failure threshold
                    critical_failures.append(result)
                    
            deviation_str = f"{result.deviation_percent:.1f}%" if not result.passed else "-"
            
            report_lines.append(
                f"| {result.metric_name} | {result.measured_value:.2f} | "
                f"{result.threshold_value:.2f} | {status} | {deviation_str} | {result.unit} |"
            )
            
        report_lines.extend([
            "",
            "## Summary",
            f"**Overall Status**: {'✅ ALL SLAs MET' if all_passed else '❌ SLA VIOLATIONS DETECTED'}",
            f"**Total Metrics**: {len(results)}",
            f"**Passed**: {sum(1 for r in results if r.passed)}",
            f"**Failed**: {sum(1 for r in results if not r.passed)}",
            ""
        ])
        
        if critical_failures:
            report_lines.extend([
                "## Critical Failures (>20% deviation)",
                ""
            ])
            for failure in critical_failures:
                report_lines.append(
                    f"- **{failure.metric_name}**: {failure.measured_value:.2f}{failure.unit} "
                    f"(threshold: {failure.threshold_value:.2f}{failure.unit}, "
                    f"deviation: {failure.deviation_percent:.1f}%)"
                )
            report_lines.append("")
            
        if not all_passed:
            report_lines.extend([
                "## Action Required",
                "⚠️ **Performance SLAs not met. Review and optimize before deployment.**",
                ""
            ])
            
        return "\n".join(report_lines)


def main():
    parser = argparse.ArgumentParser(description='Validate PRISM performance SLAs')
    parser.add_argument('results_file', help='JSON test results file')
    parser.add_argument('--max-storage-latency', type=float, default=50.0,
                        help='Maximum storage latency in ms (default: 50)')
    parser.add_argument('--min-storage-throughput', type=float, default=100.0,
                        help='Minimum storage throughput in MB/s (default: 100)')
    parser.add_argument('--max-api-response', type=float, default=200.0,
                        help='Maximum API response time in ms (default: 200)')
    parser.add_argument('--max-consensus-latency', type=float, default=200.0,
                        help='Maximum consensus latency in ms (default: 200)')
    parser.add_argument('--max-memory', type=float, default=512.0,
                        help='Maximum memory usage in MB per agent (default: 512)')
    parser.add_argument('--min-dedup-ratio', type=float, default=0.7,
                        help='Minimum deduplication ratio (default: 0.7)')
    parser.add_argument('--min-compression-ratio', type=float, default=0.6,
                        help='Minimum compression ratio (default: 0.6)')
    parser.add_argument('--output-format', choices=['text', 'json'], default='text',
                        help='Output format (default: text)')
    parser.add_argument('--verbose', '-v', action='store_true',
                        help='Enable verbose logging')
    
    args = parser.parse_args()
    
    # Configure logging
    log_level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=log_level,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Define SLA thresholds
    thresholds = [
        SLAThreshold('storage_latency_ms', args.max_storage_latency, 'max', 'ms'),
        SLAThreshold('storage_throughput_mbs', args.min_storage_throughput, 'min', 'MB/s'),
        SLAThreshold('api_response_ms', args.max_api_response, 'max', 'ms'),
        SLAThreshold('consensus_latency_ms', args.max_consensus_latency, 'max', 'ms'),
        SLAThreshold('memory_usage_mb', args.max_memory, 'max', 'MB'),
        SLAThreshold('deduplication_ratio', args.min_dedup_ratio, 'min', 'ratio'),
        SLAThreshold('compression_ratio', args.min_compression_ratio, 'min', 'ratio'),
    ]
    
    # Initialize validator
    validator = SLAValidator()
    
    # Parse test results
    test_data = validator.parse_test_results(args.results_file)
    
    # Extract metrics
    metrics = validator.extract_performance_metrics(test_data)
    
    if not metrics:
        logging.error("No performance metrics found in test results")
        sys.exit(1)
        
    # Validate SLAs
    results = validator.validate_sla(metrics, thresholds)
    
    if not results:
        logging.error("No SLA validations performed")
        sys.exit(1)
        
    # Generate and output report
    if args.output_format == 'json':
        json_output = {
            'timestamp': __import__('datetime').datetime.utcnow().isoformat(),
            'overall_passed': all(r.passed for r in results),
            'total_metrics': len(results),
            'passed_metrics': sum(1 for r in results if r.passed),
            'failed_metrics': sum(1 for r in results if not r.passed),
            'results': [
                {
                    'metric_name': r.metric_name,
                    'measured_value': r.measured_value,
                    'threshold_value': r.threshold_value,
                    'passed': r.passed,
                    'deviation_percent': r.deviation_percent,
                    'unit': r.unit
                }
                for r in results
            ]
        }
        print(json.dumps(json_output, indent=2))
    else:
        report = validator.generate_report(results)
        print(report)
        
    # Exit with error code if any SLAs failed
    if not all(r.passed for r in results):
        sys.exit(1)
        
    sys.exit(0)


if __name__ == '__main__':
    main()